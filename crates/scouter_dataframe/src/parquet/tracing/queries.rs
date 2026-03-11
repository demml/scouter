use crate::error::TraceEngineError;
use crate::parquet::utils::match_attr_expr;
use arrow::array::RecordBatch;
use arrow::array::{
    BinaryArray, Int32Array, Int64Array, ListArray, MapArray, StringArray,
    TimestampMicrosecondArray,
};
use arrow::compute::cast;
use arrow::datatypes::DataType;
use arrow_array::Array;
use chrono::{DateTime, Datelike, TimeZone, Utc};
use datafusion::common::JoinType;
use datafusion::logical_expr::{cast as df_cast, col, lit, when, SortExpr};
use datafusion::prelude::*;
use datafusion::scalar::ScalarValue;
use mini_moka::sync::Cache;
use scouter_types::sql::{TraceFilters, TraceMetricBucket, TraceSpan};
use scouter_types::{Attribute, SpanEvent, SpanId, SpanLink, TraceId};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, instrument};

/// Days from year-0001 to Unix epoch (1970-01-01), used to convert chrono → Arrow Date32.
const UNIX_EPOCH_DAYS: i32 = 719_163;

/// Build a typed `Timestamp(Microsecond, UTC)` literal for DataFusion predicates.
///
/// Using this instead of `lit(dt.to_rfc3339())` ensures the predicate type matches
/// the column type exactly, enabling Parquet row-group min/max pruning.
#[inline]
pub(crate) fn ts_lit(dt: &DateTime<Utc>) -> Expr {
    lit(ScalarValue::TimestampMicrosecond(
        Some(dt.timestamp_micros()),
        Some("UTC".into()),
    ))
}

/// Build a typed `Date32` literal for DataFusion partition pruning predicates.
///
/// Partition-level filters are evaluated at directory granularity — DataFusion skips
/// entire `partition_date=YYYY-MM-DD/` directories before reading any file statistics.
#[inline]
pub(crate) fn date_lit(dt: &DateTime<Utc>) -> Expr {
    let days = dt.date_naive().num_days_from_ce() - UNIX_EPOCH_DAYS;
    lit(ScalarValue::Date32(Some(days)))
}

// Column name constants
pub const START_TIME_COL: &str = "start_time";
pub const PARTITION_DATE_COL: &str = "partition_date";
pub const END_TIME_COL: &str = "end_time";
pub const SERVICE_NAME_COL: &str = "service_name";
pub const TRACE_ID_COL: &str = "trace_id";
pub const SPAN_ID_COL: &str = "span_id";
pub const PARENT_SPAN_ID_COL: &str = "parent_span_id";
pub const SPAN_NAME_COL: &str = "span_name";
pub const SPAN_KIND_COL: &str = "span_kind";
pub const DURATION_MS_COL: &str = "duration_ms";
pub const STATUS_CODE_COL: &str = "status_code";
pub const STATUS_MESSAGE_COL: &str = "status_message";
pub const ATTRIBUTES_COL: &str = "attributes";
pub const EVENTS_COL: &str = "events";
pub const LINKS_COL: &str = "links";
pub const INPUT_COL: &str = "input";
pub const OUTPUT_COL: &str = "output";
pub const SEARCH_BLOB_COL: &str = "search_blob";
pub const ENTITY_ID_COL: &str = "entity_id";
pub const SPAN_TABLE_NAME: &str = "trace_spans";

const SUMMARY_TABLE_NAME: &str = "trace_summaries";
const ERROR_COUNT_COL: &str = "error_count";
const ENTITY_IDS_COL: &str = "entity_ids";
const QUEUE_IDS_COL: &str = "queue_ids";

/// Columns needed to reconstruct a `TraceSpan` (all fields except search_blob).
const SPAN_COLUMNS: &[&str] = &[
    TRACE_ID_COL,
    SPAN_ID_COL,
    PARENT_SPAN_ID_COL,
    SERVICE_NAME_COL,
    SPAN_NAME_COL,
    SPAN_KIND_COL,
    START_TIME_COL,
    END_TIME_COL,
    DURATION_MS_COL,
    STATUS_CODE_COL,
    STATUS_MESSAGE_COL,
    ATTRIBUTES_COL,
    EVENTS_COL,
    LINKS_COL,
    INPUT_COL,
    OUTPUT_COL,
];

/// Flat span extracted from Arrow — no hierarchy fields.
/// `build_span_tree()` assigns depth, span_order, path, root_span_id.
struct FlatSpan {
    trace_id: [u8; 16],
    span_id: [u8; 8],
    parent_span_id: Option<[u8; 8]>,
    service_name: String,
    span_name: String,
    span_kind: Option<String>,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    duration_ms: i64,
    status_code: i32,
    status_message: Option<String>,
    attributes: Vec<Attribute>,
    events: Vec<SpanEvent>,
    links: Vec<SpanLink>,
    input: Option<serde_json::Value>,
    output: Option<serde_json::Value>,
}

struct TraceQueryBuilder {
    df: DataFrame,
}

impl TraceQueryBuilder {
    async fn set_table(
        ctx: Arc<SessionContext>,
        table_name: &str,
    ) -> Result<Self, TraceEngineError> {
        let df = ctx
            .table(table_name)
            .await
            .inspect_err(|e| error!("Failed to load table {}: {}", table_name, e))?;
        Ok(Self { df })
    }

    fn select_columns(mut self, columns: &[&str]) -> Result<Self, TraceEngineError> {
        self.df = self.df.select_columns(columns)?;
        Ok(self)
    }

    fn add_filter(mut self, expr: Expr) -> Result<Self, TraceEngineError> {
        self.df = self.df.filter(expr)?;
        Ok(self)
    }

    fn add_sort(mut self, sort: Vec<SortExpr>) -> Result<Self, TraceEngineError> {
        self.df = self.df.sort(sort)?;
        Ok(self)
    }

    fn with_limit(mut self, n: Option<usize>) -> Result<Self, TraceEngineError> {
        self.df = self.df.limit(0, n)?;
        Ok(self)
    }

    async fn execute(self) -> Result<Vec<RecordBatch>, TraceEngineError> {
        let batches = self
            .df
            .collect()
            .await
            .inspect_err(|e| error!("Failed to collect query results: {}", e))?;
        Ok(batches)
    }
}

/// Extract attributes from a MapArray at a given row index.
fn extract_attributes(map_array: &MapArray, row_idx: usize) -> Vec<Attribute> {
    if map_array.is_null(row_idx) {
        return Vec::new();
    }
    let entry = map_array.value(row_idx);
    let struct_array = entry
        .as_any()
        .downcast_ref::<arrow::array::StructArray>()
        .unwrap();
    let keys_arr = cast(struct_array.column(0).as_ref(), &DataType::Utf8).unwrap();
    let keys = keys_arr.as_any().downcast_ref::<StringArray>().unwrap();
    let values_arr = cast(struct_array.column(1).as_ref(), &DataType::Utf8).unwrap();
    let values = values_arr.as_any().downcast_ref::<StringArray>().unwrap();

    (0..struct_array.len())
        .map(|i| Attribute {
            key: keys.value(i).to_string(),
            value: serde_json::from_str(values.value(i)).unwrap_or(serde_json::Value::Null),
        })
        .collect()
}

/// Extract SpanEvents from a ListArray at a given row index.
fn extract_events(list_array: &ListArray, row_idx: usize) -> Vec<SpanEvent> {
    if list_array.is_null(row_idx) {
        return Vec::new();
    }
    let values = list_array.value(row_idx);
    let struct_array = values
        .as_any()
        .downcast_ref::<arrow::array::StructArray>()
        .unwrap();

    let names_arr = cast(
        struct_array
            .column_by_name("name")
            .expect("event name col")
            .as_ref(),
        &DataType::Utf8,
    )
    .expect("event name cast");
    let names = names_arr
        .as_any()
        .downcast_ref::<StringArray>()
        .expect("event name StringArray");
    let timestamps = struct_array
        .column_by_name("timestamp")
        .and_then(|c| c.as_any().downcast_ref::<TimestampMicrosecondArray>())
        .expect("event timestamp should be TimestampMicrosecondArray");
    let attrs = struct_array
        .column_by_name("attributes")
        .and_then(|c| c.as_any().downcast_ref::<MapArray>())
        .expect("event attributes should be MapArray");
    // Delta Lake maps UInt32 → Integer (Int32); DataFusion returns Int32Array on read.
    let dropped = struct_array
        .column_by_name("dropped_attributes_count")
        .and_then(|c| c.as_any().downcast_ref::<arrow::array::Int32Array>())
        .expect("dropped_attributes_count should be Int32Array");

    (0..struct_array.len())
        .map(|i| {
            let micros = timestamps.value(i);
            let secs = micros / 1_000_000;
            let nanos = ((micros % 1_000_000) * 1_000) as u32;
            let ts = Utc.timestamp_opt(secs, nanos).unwrap();
            SpanEvent {
                name: names.value(i).to_string(),
                timestamp: ts,
                attributes: extract_attributes(attrs, i),
                dropped_attributes_count: dropped.value(i) as u32,
            }
        })
        .collect()
}

/// Extract SpanLinks from a ListArray at a given row index.
fn extract_links(list_array: &ListArray, row_idx: usize) -> Vec<SpanLink> {
    if list_array.is_null(row_idx) {
        return Vec::new();
    }
    let values = list_array.value(row_idx);
    let struct_array = values
        .as_any()
        .downcast_ref::<arrow::array::StructArray>()
        .unwrap();

    // Cast FixedSizeBinary → Binary and any string variant → Utf8 for type-stable access.
    let trace_id_arr = cast(
        struct_array
            .column_by_name("trace_id")
            .expect("link trace_id col")
            .as_ref(),
        &DataType::Binary,
    )
    .expect("link trace_id cast");
    let trace_ids = trace_id_arr
        .as_any()
        .downcast_ref::<BinaryArray>()
        .expect("link trace_id BinaryArray");
    let span_id_arr = cast(
        struct_array
            .column_by_name("span_id")
            .expect("link span_id col")
            .as_ref(),
        &DataType::Binary,
    )
    .expect("link span_id cast");
    let span_ids = span_id_arr
        .as_any()
        .downcast_ref::<BinaryArray>()
        .expect("link span_id BinaryArray");
    let trace_state_arr = cast(
        struct_array
            .column_by_name("trace_state")
            .expect("link trace_state col")
            .as_ref(),
        &DataType::Utf8,
    )
    .expect("link trace_state cast");
    let trace_states = trace_state_arr
        .as_any()
        .downcast_ref::<StringArray>()
        .expect("link trace_state StringArray");
    let attrs = struct_array
        .column_by_name("attributes")
        .and_then(|c| c.as_any().downcast_ref::<MapArray>())
        .expect("link attributes should be MapArray");
    // Delta Lake maps UInt32 → Integer (Int32); DataFusion returns Int32Array on read.
    let dropped = struct_array
        .column_by_name("dropped_attributes_count")
        .and_then(|c| c.as_any().downcast_ref::<arrow::array::Int32Array>())
        .expect("link dropped_attributes_count should be Int32Array");

    (0..struct_array.len())
        .map(|i| {
            let tid_bytes: [u8; 16] = trace_ids.value(i).try_into().expect("16 bytes");
            let sid_bytes: [u8; 8] = span_ids.value(i).try_into().expect("8 bytes");
            SpanLink {
                trace_id: TraceId::from_bytes(tid_bytes).to_hex(),
                span_id: SpanId::from_bytes(sid_bytes).to_hex(),
                trace_state: trace_states.value(i).to_string(),
                attributes: extract_attributes(attrs, i),
                dropped_attributes_count: dropped.value(i) as u32,
            }
        })
        .collect()
}

/// Convert Arrow RecordBatches to `FlatSpan` intermediate structs.
fn batches_to_flat_spans(batches: Vec<RecordBatch>) -> Result<Vec<FlatSpan>, TraceEngineError> {
    let mut result = Vec::new();

    for batch in &batches {
        let schema = batch.schema();

        macro_rules! col_idx {
            ($name:expr) => {
                schema.index_of($name).map_err(|_| {
                    TraceEngineError::BatchConversion(format!("Missing column: {}", $name))
                })?
            };
        }

        // Cast FixedSizeBinary → Binary: table_provider() may return either type depending
        // on whether DataFusion resolves the Delta schema or the Arrow Parquet file metadata.
        // cast() is zero-copy for fixed-size → variable-length binary reinterpretation.
        let trace_id_arr = cast(
            batch.column(col_idx!("trace_id")).as_ref(),
            &DataType::Binary,
        )
        .map_err(|e| TraceEngineError::BatchConversion(format!("trace_id cast: {e}")))?;
        let trace_id_col = trace_id_arr
            .as_any()
            .downcast_ref::<BinaryArray>()
            .ok_or_else(|| TraceEngineError::BatchConversion("trace_id not BinaryArray".into()))?;

        let span_id_arr = cast(
            batch.column(col_idx!("span_id")).as_ref(),
            &DataType::Binary,
        )
        .map_err(|e| TraceEngineError::BatchConversion(format!("span_id cast: {e}")))?;
        let span_id_col = span_id_arr
            .as_any()
            .downcast_ref::<BinaryArray>()
            .ok_or_else(|| TraceEngineError::BatchConversion("span_id not BinaryArray".into()))?;

        let parent_id_arr = cast(
            batch.column(col_idx!("parent_span_id")).as_ref(),
            &DataType::Binary,
        )
        .map_err(|e| TraceEngineError::BatchConversion(format!("parent_span_id cast: {e}")))?;
        let parent_id_col = parent_id_arr
            .as_any()
            .downcast_ref::<BinaryArray>()
            .ok_or_else(|| {
                TraceEngineError::BatchConversion("parent_span_id not BinaryArray".into())
            })?;
        // Dictionary(Int32/Int8, Utf8) comes back as DictionaryArray from Parquet schema path;
        // cast to Utf8 normalizes to StringArray regardless of schema path.
        let svc_arr = cast(
            batch.column(col_idx!("service_name")).as_ref(),
            &DataType::Utf8,
        )
        .map_err(|e| TraceEngineError::BatchConversion(format!("service_name cast: {e}")))?;
        let svc_col = svc_arr
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| {
                TraceEngineError::BatchConversion("service_name not StringArray".into())
            })?;
        let span_name_arr = cast(
            batch.column(col_idx!("span_name")).as_ref(),
            &DataType::Utf8,
        )
        .map_err(|e| TraceEngineError::BatchConversion(format!("span_name cast: {e}")))?;
        let span_name_col = span_name_arr
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| TraceEngineError::BatchConversion("span_name not StringArray".into()))?;
        let span_kind_arr = cast(
            batch.column(col_idx!("span_kind")).as_ref(),
            &DataType::Utf8,
        )
        .map_err(|e| TraceEngineError::BatchConversion(format!("span_kind cast: {e}")))?;
        let span_kind_col = span_kind_arr
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| TraceEngineError::BatchConversion("span_kind not StringArray".into()))?;
        let start_col = batch
            .column(col_idx!("start_time"))
            .as_any()
            .downcast_ref::<TimestampMicrosecondArray>()
            .ok_or_else(|| TraceEngineError::BatchConversion("start_time not Timestamp".into()))?;
        let end_col = batch
            .column(col_idx!("end_time"))
            .as_any()
            .downcast_ref::<TimestampMicrosecondArray>()
            .ok_or_else(|| TraceEngineError::BatchConversion("end_time not Timestamp".into()))?;
        let dur_col = batch
            .column(col_idx!("duration_ms"))
            .as_any()
            .downcast_ref::<Int64Array>()
            .ok_or_else(|| TraceEngineError::BatchConversion("duration_ms not Int64".into()))?;
        let sc_col = batch
            .column(col_idx!("status_code"))
            .as_any()
            .downcast_ref::<Int32Array>()
            .ok_or_else(|| TraceEngineError::BatchConversion("status_code not Int32".into()))?;
        let sm_arr = cast(
            batch.column(col_idx!("status_message")).as_ref(),
            &DataType::Utf8,
        )
        .map_err(|e| TraceEngineError::BatchConversion(format!("status_message cast: {e}")))?;
        let sm_col = sm_arr
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| {
                TraceEngineError::BatchConversion("status_message not StringArray".into())
            })?;
        let attrs_col = batch
            .column(col_idx!("attributes"))
            .as_any()
            .downcast_ref::<MapArray>()
            .ok_or_else(|| TraceEngineError::BatchConversion("attributes not MapArray".into()))?;
        let events_col = batch
            .column(col_idx!("events"))
            .as_any()
            .downcast_ref::<ListArray>()
            .ok_or_else(|| TraceEngineError::BatchConversion("events not ListArray".into()))?;
        let links_col = batch
            .column(col_idx!("links"))
            .as_any()
            .downcast_ref::<ListArray>()
            .ok_or_else(|| TraceEngineError::BatchConversion("links not ListArray".into()))?;
        // Utf8View stored in Parquet may come back as StringViewArray; cast to Utf8 normalizes.
        let input_arr = cast(batch.column(col_idx!("input")).as_ref(), &DataType::Utf8)
            .map_err(|e| TraceEngineError::BatchConversion(format!("input cast: {e}")))?;
        let input_col = input_arr
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| TraceEngineError::BatchConversion("input not StringArray".into()))?;
        let output_arr = cast(batch.column(col_idx!("output")).as_ref(), &DataType::Utf8)
            .map_err(|e| TraceEngineError::BatchConversion(format!("output cast: {e}")))?;
        let output_col = output_arr
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| TraceEngineError::BatchConversion("output not StringArray".into()))?;

        for i in 0..batch.num_rows() {
            let tid_bytes: [u8; 16] = trace_id_col
                .value(i)
                .try_into()
                .map_err(|_| TraceEngineError::BatchConversion("trace_id bad length".into()))?;
            let sid_bytes: [u8; 8] = span_id_col
                .value(i)
                .try_into()
                .map_err(|_| TraceEngineError::BatchConversion("span_id bad length".into()))?;

            let parent_id = if parent_id_col.is_null(i) {
                None
            } else {
                let bytes: [u8; 8] = parent_id_col.value(i).try_into().map_err(|_| {
                    TraceEngineError::BatchConversion("parent_span_id bad length".into())
                })?;
                Some(bytes)
            };

            let micros_start = start_col.value(i);
            let start_time = Utc
                .timestamp_opt(
                    micros_start / 1_000_000,
                    ((micros_start % 1_000_000) * 1_000) as u32,
                )
                .unwrap();
            let micros_end = end_col.value(i);
            let end_time = Utc
                .timestamp_opt(
                    micros_end / 1_000_000,
                    ((micros_end % 1_000_000) * 1_000) as u32,
                )
                .unwrap();

            let input = if input_col.is_null(i) {
                None
            } else {
                serde_json::from_str(input_col.value(i)).ok()
            };
            let output = if output_col.is_null(i) {
                None
            } else {
                serde_json::from_str(output_col.value(i)).ok()
            };

            result.push(FlatSpan {
                trace_id: tid_bytes,
                span_id: sid_bytes,
                parent_span_id: parent_id,
                service_name: svc_col.value(i).to_string(),
                span_name: span_name_col.value(i).to_string(),
                span_kind: if span_kind_col.is_null(i) {
                    None
                } else {
                    Some(span_kind_col.value(i).to_string())
                },
                start_time,
                end_time,
                duration_ms: dur_col.value(i),
                status_code: sc_col.value(i),
                status_message: if sm_col.is_null(i) {
                    None
                } else {
                    Some(sm_col.value(i).to_string())
                },
                attributes: extract_attributes(attrs_col, i),
                events: extract_events(events_col, i),
                links: extract_links(links_col, i),
                input,
                output,
            });
        }
    }

    Ok(result)
}

/// Build a `Vec<TraceSpan>` from flat spans by computing hierarchy via DFS traversal.
///
/// Assigns `depth`, `span_order`, `path`, and `root_span_id` — the same fields that
/// Postgres computed via a recursive CTE. Spans are returned in DFS order (span_order ascending).
fn build_span_tree(spans: Vec<FlatSpan>) -> Vec<TraceSpan> {
    if spans.is_empty() {
        return Vec::new();
    }

    // Find root span (no parent)
    let root_span_id_hex = spans
        .iter()
        .find(|s| s.parent_span_id.is_none())
        .map(|s| SpanId::from_bytes(s.span_id).to_hex())
        .unwrap_or_else(|| {
            // All spans have parents — use first span's parent as synthetic root
            SpanId::from_bytes(spans[0].span_id).to_hex()
        });

    // Build children map: parent_span_id → Vec<index>
    let mut children: HashMap<[u8; 8], Vec<usize>> = HashMap::new();
    let mut root_indices: Vec<usize> = Vec::new();

    for (i, span) in spans.iter().enumerate() {
        if let Some(pid) = span.parent_span_id {
            children.entry(pid).or_default().push(i);
        } else {
            root_indices.push(i);
        }
    }

    // Sort root indices by start_time for deterministic ordering
    root_indices.sort_by_key(|&i| spans[i].start_time);

    let mut result: Vec<TraceSpan> = Vec::with_capacity(spans.len());
    let mut span_order: i32 = 0;

    dfs_assign(
        &root_indices,
        &spans,
        &children,
        0,
        Vec::new(),
        &root_span_id_hex,
        &mut result,
        &mut span_order,
    );

    // Attach orphan spans (parent not found in this batch)
    let visited: std::collections::HashSet<[u8; 8]> = result
        .iter()
        .filter_map(|s| {
            let v = SpanId::hex_to_bytes(&s.span_id).ok()?;
            let arr: [u8; 8] = v.try_into().ok()?;
            Some(arr)
        })
        .collect();

    for span in spans.iter() {
        if !visited.contains(&span.span_id) {
            let span_id_hex = SpanId::from_bytes(span.span_id).to_hex();
            result.push(flat_to_trace_span(
                span,
                &span_id_hex,
                &root_span_id_hex,
                0,
                vec![span_id_hex.clone()],
                span_order,
            ));
            span_order += 1;
        }
    }

    result
}

#[allow(clippy::too_many_arguments)]
fn dfs_assign(
    indices: &[usize],
    spans: &[FlatSpan],
    children: &HashMap<[u8; 8], Vec<usize>>,
    depth: i32,
    path_so_far: Vec<String>,
    root_span_id_hex: &str,
    result: &mut Vec<TraceSpan>,
    span_order: &mut i32,
) {
    for &idx in indices {
        let span = &spans[idx];
        let span_id_hex = SpanId::from_bytes(span.span_id).to_hex();

        let mut path = path_so_far.clone();
        path.push(span_id_hex.clone());

        result.push(flat_to_trace_span(
            span,
            &span_id_hex,
            root_span_id_hex,
            depth,
            path.clone(),
            *span_order,
        ));
        *span_order += 1;

        // Recurse into children, sorted by start_time
        if let Some(child_indices) = children.get(&span.span_id) {
            let mut sorted = child_indices.clone();
            sorted.sort_by_key(|&i| spans[i].start_time);
            dfs_assign(
                &sorted,
                spans,
                children,
                depth + 1,
                path,
                root_span_id_hex,
                result,
                span_order,
            );
        }
    }
}

fn flat_to_trace_span(
    span: &FlatSpan,
    span_id_hex: &str,
    root_span_id_hex: &str,
    depth: i32,
    path: Vec<String>,
    span_order: i32,
) -> TraceSpan {
    TraceSpan {
        trace_id: TraceId::from_bytes(span.trace_id).to_hex(),
        span_id: span_id_hex.to_string(),
        parent_span_id: span.parent_span_id.map(|b| SpanId::from_bytes(b).to_hex()),
        span_name: span.span_name.clone(),
        span_kind: span.span_kind.clone(),
        start_time: span.start_time,
        end_time: span.end_time,
        duration_ms: span.duration_ms,
        status_code: span.status_code,
        status_message: span.status_message.clone(),
        attributes: span.attributes.clone(),
        events: span.events.clone(),
        links: span.links.clone(),
        depth,
        path,
        root_span_id: root_span_id_hex.to_string(),
        service_name: span.service_name.clone(),
        span_order,
        input: span.input.clone(),
        output: span.output.clone(),
    }
}

/// Normalize an attribute filter string for search_blob LIKE matching.
///
/// Converts `key:value` separator to `key=value` (standardized format) so filters match
/// the pipe-bounded `|key=value|` blob written by `build_search_blob()`.
///
/// Note: URL-like patterns (`http://`) are left unchanged to avoid breaking URL values.
pub(crate) fn normalize_attr_filter(filter: &str) -> String {
    let normalized = match filter.find(':') {
        Some(pos) if !filter[pos..].starts_with("://") => {
            format!("{}={}", &filter[..pos], &filter[pos + 1..])
        }
        _ => filter.to_string(),
    };
    format!("%{}%", normalized)
}

/// High-performance query patterns for Delta Lake trace storage.
///
/// Time predicates are always applied FIRST to enable Delta Lake partition pruning.
/// `span_cache` provides sub-millisecond repeat reads for trace detail clicks.
/// `metrics_cache` provides sub-millisecond repeat reads for dashboard metric charts.
pub struct TraceQueries {
    ctx: Arc<SessionContext>,
    /// LRU cache keyed by 16-byte trace ID. TTL=5 min — archived span data is immutable.
    span_cache: Cache<[u8; 16], Arc<Vec<TraceSpan>>>,
    /// LRU cache keyed by hash of (service, start, end, interval, filters, entity).
    /// TTL=60s — short enough to reflect new archive writes, long enough to absorb UI refreshes.
    metrics_cache: Cache<u64, Arc<Vec<TraceMetricBucket>>>,
}

/// Compute a stable u64 cache key from all `get_trace_metrics` parameters.
fn metrics_cache_key(
    service_name: Option<&str>,
    start_time: &DateTime<Utc>,
    end_time: &DateTime<Utc>,
    bucket_interval: &str,
    attribute_filters: Option<&[String]>,
    entity_uid: Option<&str>,
) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    service_name.hash(&mut h);
    start_time.timestamp_micros().hash(&mut h);
    end_time.timestamp_micros().hash(&mut h);
    bucket_interval.hash(&mut h);
    attribute_filters.hash(&mut h);
    entity_uid.hash(&mut h);
    h.finish()
}

impl TraceQueries {
    pub fn new(ctx: Arc<SessionContext>) -> Self {
        let span_cache = Cache::builder()
            .max_capacity(1_000)
            .time_to_live(Duration::from_secs(300))
            .build();
        let metrics_cache = Cache::builder()
            .max_capacity(500)
            .time_to_live(Duration::from_secs(60))
            .build();
        Self {
            ctx,
            span_cache,
            metrics_cache,
        }
    }

    /// Get all spans for a trace, reconstructed as a tree with hierarchy fields populated.
    ///
    /// # Arguments
    /// * `trace_id_bytes` - Raw 16-byte trace ID
    /// * `service_name` - Optional service filter
    /// * `start_time` - Optional lower time bound (applied FIRST for partition pruning)
    /// * `end_time` - Optional upper time bound
    /// * `limit` - Optional row limit
    ///
    /// When `trace_id_bytes` is 16 bytes, results are cached for 5 minutes — repeat detail
    /// clicks (common in the UI) return in <1µs without hitting Delta Lake.
    #[instrument(skip_all)]
    pub async fn get_trace_spans(
        &self,
        trace_id_bytes: Option<&[u8]>,
        service_name: Option<&str>,
        start_time: Option<&DateTime<Utc>>,
        end_time: Option<&DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<Vec<TraceSpan>, TraceEngineError> {
        // Cache lookup for by-id trace detail queries (the hot interactive path).
        if let Some(tid) = trace_id_bytes {
            if let Ok(key) = <[u8; 16]>::try_from(tid) {
                if let Some(cached) = self.span_cache.get(&key) {
                    return Ok((*cached).clone());
                }

                let result = self
                    .query_spans(Some(tid), service_name, start_time, end_time, limit)
                    .await?;
                self.span_cache.insert(key, Arc::new(result.clone()));
                return Ok(result);
            }
        }

        // No trace_id or non-16-byte ID — uncached scan path (time/service/attribute queries).
        self.query_spans(trace_id_bytes, service_name, start_time, end_time, limit)
            .await
    }

    /// Execute the actual DataFusion query without cache logic.
    pub async fn query_spans(
        &self,
        trace_id_bytes: Option<&[u8]>,
        service_name: Option<&str>,
        start_time: Option<&DateTime<Utc>>,
        end_time: Option<&DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<Vec<TraceSpan>, TraceEngineError> {
        let mut builder = TraceQueryBuilder::set_table(self.ctx.clone(), SPAN_TABLE_NAME).await?;

        // Partition filters FIRST — eliminates whole partition_date=YYYY-MM-DD/ directories
        // at directory level before any file metadata or Parquet statistics are read.
        if let Some(start) = start_time {
            builder = builder.add_filter(col(PARTITION_DATE_COL).gt_eq(date_lit(start)))?;
        }
        if let Some(end) = end_time {
            builder = builder.add_filter(col(PARTITION_DATE_COL).lt_eq(date_lit(end)))?;
        }

        // Row-group level pruning — typed Timestamp literals enable Parquet min/max pruning
        // within the partition directories that survived the directory-level filter above.
        if let Some(start) = start_time {
            builder = builder.add_filter(col(START_TIME_COL).gt_eq(ts_lit(start)))?;
        }
        if let Some(end) = end_time {
            builder = builder.add_filter(col(START_TIME_COL).lt(ts_lit(end)))?;
        }

        if let Some(tid) = trace_id_bytes {
            builder = builder.add_filter(col(TRACE_ID_COL).eq(lit(tid)))?;
        }

        if let Some(svc) = service_name {
            builder = builder.add_filter(col(SERVICE_NAME_COL).eq(lit(svc)))?;
        }

        builder = builder.select_columns(SPAN_COLUMNS)?;

        // Sort by start_time for stable DFS input; tree builder assigns span_order
        builder = builder.add_sort(vec![col(START_TIME_COL).sort(true, true)])?;
        builder = builder.with_limit(limit)?;

        let batches = builder.execute().await?;

        info!(
            "Queried {} raw spans across {} batches from Delta Lake",
            batches.iter().map(|b| b.num_rows()).sum::<usize>(),
            batches.len()
        );

        let flat_spans = batches_to_flat_spans(batches)?;
        Ok(build_span_tree(flat_spans))
    }

    /// Get trace metrics over a time range, bucketed by the given interval string.
    ///
    /// `bucket_interval` must be a valid DataFusion `DATE_TRUNC` precision unit:
    /// `"second"`, `"minute"`, `"hour"`, `"day"`, `"week"`, `"month"`, `"year"`.
    ///
    /// Matches Postgres logic: trace duration = `MAX(end_time) - MIN(start_time)` across all
    /// spans of a trace (not per-span `duration_ms`). Root service is the service of the span
    /// where `parent_span_id IS NULL`. Service filter applies to root spans only.
    ///
    /// `attribute_filters` is a list of `"key:value"` strings OR-matched against `search_blob`.
    /// `entity_trace_ids` is an optional pre-resolved list of binary trace IDs (16 bytes each).
    #[instrument(skip_all)]
    pub async fn get_trace_metrics(
        &self,
        service_name: Option<&str>,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        bucket_interval: &str,
        attribute_filters: Option<&[String]>,
        entity_uid: Option<&str>,
    ) -> Result<Vec<TraceMetricBucket>, TraceEngineError> {
        // Cache hit: return immediately without touching Delta Lake.
        let cache_key = metrics_cache_key(
            service_name,
            &start_time,
            &end_time,
            bucket_interval,
            attribute_filters,
            entity_uid,
        );
        if let Some(cached) = self.metrics_cache.get(&cache_key) {
            return Ok((*cached).clone());
        }

        const VALID_INTERVALS: &[&str] =
            &["second", "minute", "hour", "day", "week", "month", "year"];
        if !VALID_INTERVALS.contains(&bucket_interval) {
            return Err(TraceEngineError::UnsupportedOperation(format!(
                "Invalid bucket_interval '{}'. Must be one of: {}",
                bucket_interval,
                VALID_INTERVALS.join(", ")
            )));
        }

        // ── Phase 1: Spans base DataFrame — time-first for partition + row-group pruning ──
        let mut spans_df = self
            .ctx
            .table(SPAN_TABLE_NAME)
            .await
            .map_err(TraceEngineError::DatafusionError)?;

        // Partition directory pruning — eliminates whole YYYY-MM-DD/ directories before
        // DataFusion reads a single file's metadata or Parquet column statistics.
        spans_df = spans_df.filter(col(PARTITION_DATE_COL).gt_eq(date_lit(&start_time)))?;
        spans_df = spans_df.filter(col(PARTITION_DATE_COL).lt_eq(date_lit(&end_time)))?;

        // Row-group pruning — typed Timestamp(Microsecond, UTC) literals let DataFusion
        // use Parquet column min/max stats within the surviving partition directories.
        spans_df = spans_df.filter(col(START_TIME_COL).gt_eq(ts_lit(&start_time)))?;
        spans_df = spans_df.filter(col(START_TIME_COL).lt(ts_lit(&end_time)))?;

        // ── Phase 2: Entity filter — optional INNER JOIN against summary table ──
        //
        // Resolves the set of matching trace IDs from the summary table (time-first),
        // then INNER JOIN into spans.  Replaces the `entity_traces` CTE + join.
        if let Some(uid) = entity_uid {
            let mut entity_df = self
                .ctx
                .table(SUMMARY_TABLE_NAME)
                .await
                .map_err(TraceEngineError::DatafusionError)?;

            // Summary-side time pruning (same partition-pruning principle as spans).
            entity_df = entity_df.filter(col(START_TIME_COL).gt_eq(ts_lit(&start_time)))?;
            entity_df = entity_df.filter(col(START_TIME_COL).lt(ts_lit(&end_time)))?;
            entity_df = entity_df.filter(datafusion::functions_nested::expr_fn::array_has(
                col(ENTITY_IDS_COL),
                lit(uid),
            ))?;

            // Alias to avoid ambiguous `trace_id` column in the JOIN output schema.
            let entity_df = entity_df
                .select(vec![col(TRACE_ID_COL).alias("_entity_tid")])?
                .distinct()?;

            spans_df = spans_df.join(
                entity_df,
                JoinType::Inner,
                &[TRACE_ID_COL],
                &["_entity_tid"],
                None,
            )?;
        }

        // ── Phase 3: trace_level — aggregate per-trace ───────────────────────
        //
        // Replaces the `trace_level` CTE:
        //   MIN(start_time) → trace_start
        //   MAX(end_time)   → trace_end (NULL when all end_times are NULL)
        //   MAX(CASE WHEN parent_span_id IS NULL THEN service_name END) → root_service
        //   MAX(status_code) → status_code
        //   [MAX(CAST(match_attr OR-chain AS INT64)) → attr_match]  ← single-scan attr filter
        //
        // CASE WHEN parent_span_id IS NULL THEN CAST(service_name AS Utf8) END:
        // The root span is the one with no parent; MAX picks the single non-NULL value
        // across all spans for a given trace.
        use datafusion::functions::expr_fn::date_trunc;
        use datafusion::functions_aggregate::expr_fn::approx_percentile_cont;
        use datafusion::functions_aggregate::expr_fn::{avg, count, max, min};

        let root_service_case = when(
            col(PARENT_SPAN_ID_COL).is_null(),
            df_cast(col(SERVICE_NAME_COL), DataType::Utf8),
        )
        .end()?;

        let has_attr_filter = attribute_filters.is_some_and(|f| !f.is_empty());

        let mut agg_exprs: Vec<Expr> = vec![
            min(col(START_TIME_COL)).alias("trace_start"),
            max(col(END_TIME_COL)).alias("trace_end"),
            max(root_service_case).alias("root_service"),
            max(col(STATUS_CODE_COL)).alias("status_code"),
        ];

        // Attribute filter: OR-chain match_attr() calls over search_blob, cast to Int64,
        // then MAX to get 1 if any span in the trace matched.
        // Single-pass: avoids a second table scan for attribute filtering.
        // Post-aggregate .filter(attr_match = 1) replaces the SQL HAVING clause.
        if has_attr_filter {
            let filters = attribute_filters.unwrap();
            let mut match_expr: Option<Expr> = None;
            for f in filters {
                let pattern = normalize_attr_filter(f);
                let cond = match_attr_expr(col(SEARCH_BLOB_COL), lit(pattern));
                match_expr = Some(match match_expr {
                    None => cond,
                    Some(e) => e.or(cond),
                });
            }
            // CAST(bool OR-chain AS INT64): true → 1, false → 0.
            // MAX over the group gives 1 if any span matched.
            let attr_int = df_cast(match_expr.unwrap(), DataType::Int64);
            agg_exprs.push(max(attr_int).alias("attr_match"));
        }

        let mut trace_level_df = spans_df.aggregate(vec![col(TRACE_ID_COL)], agg_exprs)?;

        // HAVING attr_match = 1 — post-aggregate filter (SQL HAVING equivalent).
        if has_attr_filter {
            trace_level_df = trace_level_df.filter(col("attr_match").eq(lit(1i64)))?;
        }

        // ── Phase 4: service_filtered — duration_ms, null guard, service filter ──
        //
        // Replaces the `service_filtered` CTE.
        // Cast Timestamp(Microsecond, UTC) → Int64 gives µs since epoch;
        // subtracting gives trace duration in µs, dividing by 1_000 gives ms.
        // Computed here (after the per-trace aggregate) to avoid the DataFusion
        // restriction on duplicate aggregate expressions in the same SELECT.
        let duration_expr = (df_cast(col("trace_end"), DataType::Int64)
            - df_cast(col("trace_start"), DataType::Int64))
            / lit(1000i64);

        let mut service_filtered_df = trace_level_df
            .filter(col("trace_end").is_not_null())?
            .with_column("duration_ms", duration_expr)?;

        if let Some(svc) = service_name {
            service_filtered_df = service_filtered_df.filter(col("root_service").eq(lit(svc)))?;
        }

        // ── Phase 5: DATE_TRUNC bucket ───────────────────────────────────────
        //
        // Replaces the `bucketed` CTE.
        // date_trunc(precision_literal, timestamp_expr) — precision is a Utf8 scalar.
        let bucket_expr = date_trunc(lit(bucket_interval), col("trace_start"));
        let bucketed_df = service_filtered_df.with_column("bucket_start", bucket_expr)?;

        // ── Phase 6: Final bucketed aggregation ─────────────────────────────
        let duration_f64 = df_cast(col("duration_ms"), DataType::Float64);
        let error_rate_case =
            when(col(STATUS_CODE_COL).eq(lit(2i32)), lit(1.0f64)).otherwise(lit(0.0f64))?;

        // approx_percentile_cont in DataFusion 52: (SortExpr, percentile, limit: Option<Expr>)
        // SortExpr is col.sort(asc, nulls_first); None limit = no row-count cap.
        let final_df = bucketed_df
            .aggregate(
                vec![col("bucket_start")],
                vec![
                    count(lit(1i64)).alias("trace_count"),
                    avg(duration_f64.clone()).alias("avg_duration_ms"),
                    approx_percentile_cont(
                        duration_f64.clone().sort(true, false),
                        lit(0.50f64),
                        None,
                    )
                    .alias("p50_duration_ms"),
                    approx_percentile_cont(
                        duration_f64.clone().sort(true, false),
                        lit(0.95f64),
                        None,
                    )
                    .alias("p95_duration_ms"),
                    approx_percentile_cont(duration_f64.sort(true, false), lit(0.99f64), None)
                        .alias("p99_duration_ms"),
                    avg(error_rate_case).alias("error_rate"),
                ],
            )?
            .sort(vec![col("bucket_start").sort(true, true)])?;

        let batches = final_df
            .collect()
            .await
            .map_err(TraceEngineError::DatafusionError)?;

        let mut metrics = Vec::new();
        for batch in &batches {
            let schema = batch.schema();

            // DATE_TRUNC may return Timestamp(Nanosecond) when string literals in the WHERE
            // clause cause DataFusion to upcast the column. Cast explicitly to
            // Timestamp(Microsecond, UTC) so Arrow handles the ns→µs division correctly,
            // regardless of the sub-type returned by the query plan.
            let raw_bucket = batch.column(schema.index_of("bucket_start").unwrap());
            let bucket_arr = arrow::compute::cast(
                raw_bucket,
                &arrow::datatypes::DataType::Timestamp(
                    arrow::datatypes::TimeUnit::Microsecond,
                    Some("UTC".into()),
                ),
            )
            .map_err(|e| TraceEngineError::BatchConversion(format!("bucket_start cast: {}", e)))?;
            let bucket_col = bucket_arr
                .as_any()
                .downcast_ref::<TimestampMicrosecondArray>()
                .ok_or_else(|| TraceEngineError::BatchConversion("bucket_start".into()))?;
            let count_col = batch
                .column(schema.index_of("trace_count").unwrap())
                .as_any()
                .downcast_ref::<Int64Array>()
                .ok_or_else(|| TraceEngineError::BatchConversion("trace_count".into()))?;
            let avg_col = batch
                .column(schema.index_of("avg_duration_ms").unwrap())
                .as_any()
                .downcast_ref::<arrow::array::Float64Array>()
                .ok_or_else(|| TraceEngineError::BatchConversion("avg_duration_ms".into()))?;
            let p50_col = batch
                .column(schema.index_of("p50_duration_ms").unwrap())
                .as_any()
                .downcast_ref::<arrow::array::Float64Array>()
                .ok_or_else(|| TraceEngineError::BatchConversion("p50_duration_ms".into()))?;
            let p95_col = batch
                .column(schema.index_of("p95_duration_ms").unwrap())
                .as_any()
                .downcast_ref::<arrow::array::Float64Array>()
                .ok_or_else(|| TraceEngineError::BatchConversion("p95_duration_ms".into()))?;
            let p99_col = batch
                .column(schema.index_of("p99_duration_ms").unwrap())
                .as_any()
                .downcast_ref::<arrow::array::Float64Array>()
                .ok_or_else(|| TraceEngineError::BatchConversion("p99_duration_ms".into()))?;
            let err_col = batch
                .column(schema.index_of("error_rate").unwrap())
                .as_any()
                .downcast_ref::<arrow::array::Float64Array>()
                .ok_or_else(|| TraceEngineError::BatchConversion("error_rate".into()))?;

            for i in 0..batch.num_rows() {
                let micros = bucket_col.value(i);
                let bucket_start = DateTime::from_timestamp_micros(micros)
                    .unwrap_or_default()
                    .with_timezone(&Utc);

                metrics.push(TraceMetricBucket {
                    bucket_start,
                    trace_count: count_col.value(i),
                    avg_duration_ms: avg_col.value(i),
                    p50_duration_ms: if p50_col.is_null(i) {
                        None
                    } else {
                        Some(p50_col.value(i))
                    },
                    p95_duration_ms: if p95_col.is_null(i) {
                        None
                    } else {
                        Some(p95_col.value(i))
                    },
                    p99_duration_ms: if p99_col.is_null(i) {
                        None
                    } else {
                        Some(p99_col.value(i))
                    },
                    error_rate: err_col.value(i),
                });
            }
        }

        self.metrics_cache
            .insert(cache_key, Arc::new(metrics.clone()));
        Ok(metrics)
    }

    /// Look up traces from the summary table that match aggregate-level filters
    /// (e.g. `entity_uid`, `queue_uid`, `has_errors`) and return spans for the
    /// most-recent matching trace. The entire pipeline runs as a single DataFusion
    /// JOIN — no intermediate collection, no Postgres round-trip.
    pub async fn query_spans_from_trace_filters(
        &self,
        filters: &TraceFilters,
    ) -> Result<Vec<TraceSpan>, TraceEngineError> {
        // ── Phase 1: Summary filters (time-first for partition pruning) ─────
        let mut summary_df = self
            .ctx
            .table(SUMMARY_TABLE_NAME)
            .await
            .map_err(TraceEngineError::DatafusionError)?;

        if let Some(start) = filters.start_time {
            summary_df = summary_df.filter(col(START_TIME_COL).gt_eq(ts_lit(&start)))?;
        }
        if let Some(end) = filters.end_time {
            summary_df = summary_df.filter(col(START_TIME_COL).lt(ts_lit(&end)))?;
        }
        if let Some(ref svc) = filters.service_name {
            summary_df = summary_df.filter(col(SERVICE_NAME_COL).eq(lit(svc.as_str())))?;
        }
        match filters.has_errors {
            Some(true) => {
                summary_df = summary_df.filter(col(ERROR_COUNT_COL).gt(lit(0i64)))?;
            }
            Some(false) => {
                summary_df = summary_df.filter(col(ERROR_COUNT_COL).eq(lit(0i64)))?;
            }
            None => {}
        }
        if let Some(sc) = filters.status_code {
            summary_df = summary_df.filter(col(STATUS_CODE_COL).eq(lit(sc)))?;
        }
        if let Some(ref uid) = filters.entity_uid {
            summary_df = summary_df.filter(datafusion::functions_nested::expr_fn::array_has(
                col(ENTITY_IDS_COL),
                lit(uid.as_str()),
            ))?;
        }
        if let Some(ref uid) = filters.queue_uid {
            summary_df = summary_df.filter(datafusion::functions_nested::expr_fn::array_has(
                col(QUEUE_IDS_COL),
                lit(uid.as_str()),
            ))?;
        }

        // ── Phase 1b: Attribute filter join (keeps everything in DataFusion) ─
        if let Some(ref attr_filters) = filters.attribute_filters {
            if !attr_filters.is_empty() {
                let mut attr_df = self
                    .ctx
                    .table(SPAN_TABLE_NAME)
                    .await
                    .map_err(TraceEngineError::DatafusionError)?;

                // Time pruning on the span side
                if let Some(start) = filters.start_time {
                    attr_df = attr_df.filter(col(START_TIME_COL).gt_eq(ts_lit(&start)))?;
                }
                if let Some(end) = filters.end_time {
                    attr_df = attr_df.filter(col(START_TIME_COL).lt(ts_lit(&end)))?;
                }

                // OR-match search_blob against each filter pattern via match_attr UDF.
                // match_attr_expr is a drop-in replacement for col(..).like(lit(pattern)):
                // handles Utf8View natively and uses .contains() for LIKE '%inner%' semantics.
                let mut attr_expr: Option<Expr> = None;
                for f in attr_filters {
                    let pattern = normalize_attr_filter(f);
                    let cond = match_attr_expr(col(SEARCH_BLOB_COL), lit(pattern));
                    attr_expr = Some(match attr_expr {
                        None => cond,
                        Some(e) => e.or(cond),
                    });
                }
                if let Some(expr) = attr_expr {
                    attr_df = attr_df.filter(expr)?;
                }

                // Deduplicate and alias trace_id to avoid ambiguous column in join
                let attr_df = attr_df
                    .select(vec![col(TRACE_ID_COL).alias("_attr_tid")])?
                    .distinct()?;

                summary_df = summary_df.join(
                    attr_df,
                    JoinType::Inner,
                    &[TRACE_ID_COL],
                    &["_attr_tid"],
                    None,
                )?;
            }
        }

        // ── Phase 2: Sort DESC, limit 1, project trace_id → _match_tid ──────
        let first_trace_df = summary_df
            .sort(vec![
                col(START_TIME_COL).sort(false, false),
                col(TRACE_ID_COL).sort(false, false),
            ])?
            .limit(0, Some(1))?
            .select(vec![col(TRACE_ID_COL).alias("_match_tid")])?;

        // ── Phase 3: Spans DataFrame — partition + row-group pruning ─────────
        let mut spans_df = self
            .ctx
            .table(SPAN_TABLE_NAME)
            .await
            .map_err(TraceEngineError::DatafusionError)?;

        if let Some(start) = filters.start_time {
            spans_df = spans_df.filter(col(PARTITION_DATE_COL).gt_eq(date_lit(&start)))?;
            spans_df = spans_df.filter(col(START_TIME_COL).gt_eq(ts_lit(&start)))?;
        }
        if let Some(end) = filters.end_time {
            spans_df = spans_df.filter(col(PARTITION_DATE_COL).lt_eq(date_lit(&end)))?;
            spans_df = spans_df.filter(col(START_TIME_COL).lt(ts_lit(&end)))?;
        }
        spans_df = spans_df.select_columns(SPAN_COLUMNS)?;
        spans_df = spans_df.sort(vec![col(START_TIME_COL).sort(true, true)])?;

        // ── Phase 4: Inner join — spans filtered to the single matching trace ─
        let result_df = spans_df.join(
            first_trace_df,
            JoinType::Inner,
            &[TRACE_ID_COL],
            &["_match_tid"],
            None,
        )?;

        let batches = result_df
            .collect()
            .await
            .map_err(TraceEngineError::DatafusionError)?;

        if batches.is_empty() || batches.iter().all(|b| b.num_rows() == 0) {
            return Ok(Vec::new());
        }

        let flat_spans = batches_to_flat_spans(batches)?;
        Ok(build_span_tree(flat_spans))
    }
}
