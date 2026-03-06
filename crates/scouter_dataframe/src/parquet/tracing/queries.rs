use crate::error::TraceEngineError;
use arrow::array::RecordBatch;
use arrow::array::{
    BinaryArray, Int32Array, Int64Array, ListArray, MapArray, StringArray,
    TimestampMicrosecondArray,
};
use arrow_array::Array;
use chrono::{DateTime, TimeZone, Utc};
use datafusion::logical_expr::{col, lit, SortExpr};
use datafusion::prelude::*;
use scouter_types::sql::{TraceMetricBucket, TraceSpan};
use scouter_types::{Attribute, SpanEvent, SpanId, SpanLink, TraceId};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info, instrument};

// Column name constants
pub const START_TIME_COL: &str = "start_time";
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
pub const SPAN_TABLE_NAME: &str = "trace_spans";

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
    let keys = struct_array
        .column(0)
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();
    let values = struct_array
        .column(1)
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();

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

    let names = struct_array
        .column_by_name("name")
        .and_then(|c| c.as_any().downcast_ref::<StringArray>())
        .expect("event name should be StringArray");
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

    // Delta Lake maps FixedSizeBinary → Binary; DataFusion returns BinaryArray for nested fields.
    let trace_ids = struct_array
        .column_by_name("trace_id")
        .and_then(|c| c.as_any().downcast_ref::<BinaryArray>())
        .expect("link trace_id should be BinaryArray");
    let span_ids = struct_array
        .column_by_name("span_id")
        .and_then(|c| c.as_any().downcast_ref::<BinaryArray>())
        .expect("link span_id should be BinaryArray");
    let trace_states = struct_array
        .column_by_name("trace_state")
        .and_then(|c| c.as_any().downcast_ref::<StringArray>())
        .expect("link trace_state should be StringArray");
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

        let trace_id_col = batch
            .column(col_idx!("trace_id"))
            .as_any()
            .downcast_ref::<BinaryArray>()
            .ok_or_else(|| TraceEngineError::BatchConversion("trace_id not BinaryArray".into()))?;
        let span_id_col = batch
            .column(col_idx!("span_id"))
            .as_any()
            .downcast_ref::<BinaryArray>()
            .ok_or_else(|| TraceEngineError::BatchConversion("span_id not BinaryArray".into()))?;
        let parent_id_col = batch
            .column(col_idx!("parent_span_id"))
            .as_any()
            .downcast_ref::<BinaryArray>()
            .ok_or_else(|| {
                TraceEngineError::BatchConversion("parent_span_id not BinaryArray".into())
            })?;
        let svc_col = batch
            .column(col_idx!("service_name"))
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| {
                TraceEngineError::BatchConversion("service_name not StringArray".into())
            })?;
        let span_name_col = batch
            .column(col_idx!("span_name"))
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| TraceEngineError::BatchConversion("span_name not StringArray".into()))?;
        let span_kind_col = batch
            .column(col_idx!("span_kind"))
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
        let sm_col = batch
            .column(col_idx!("status_message"))
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
        let input_col = batch
            .column(col_idx!("input"))
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| TraceEngineError::BatchConversion("input not StringArray".into()))?;
        let output_col = batch
            .column(col_idx!("output"))
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

/// High-performance query patterns for Delta Lake trace storage.
///
/// Time predicates are always applied FIRST to enable Delta Lake partition pruning.
pub struct TraceQueries {
    ctx: Arc<SessionContext>,
}

impl TraceQueries {
    pub fn new(ctx: Arc<SessionContext>) -> Self {
        Self { ctx }
    }

    /// Get all spans for a trace, reconstructed as a tree with hierarchy fields populated.
    ///
    /// # Arguments
    /// * `trace_id_bytes` - Raw 16-byte trace ID
    /// * `service_name` - Optional service filter
    /// * `start_time` - Optional lower time bound (applied FIRST for partition pruning)
    /// * `end_time` - Optional upper time bound
    /// * `limit` - Optional row limit
    #[instrument(skip_all)]
    pub async fn get_trace_spans(
        &self,
        trace_id_bytes: Option<&[u8]>,
        service_name: Option<&str>,
        start_time: Option<&DateTime<Utc>>,
        end_time: Option<&DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Result<Vec<TraceSpan>, TraceEngineError> {
        let mut builder = TraceQueryBuilder::set_table(self.ctx.clone(), SPAN_TABLE_NAME).await?;
        builder = builder.select_columns(SPAN_COLUMNS)?;

        // Time predicates FIRST — enables Delta Lake partition pruning
        if let Some(start) = start_time {
            builder = builder.add_filter(col(START_TIME_COL).gt_eq(lit(start.to_rfc3339())))?;
        }
        if let Some(end) = end_time {
            builder = builder.add_filter(col(START_TIME_COL).lt(lit(end.to_rfc3339())))?;
        }

        if let Some(tid) = trace_id_bytes {
            builder = builder.add_filter(col(TRACE_ID_COL).eq(lit(tid)))?;
        }

        if let Some(svc) = service_name {
            builder = builder.add_filter(col(SERVICE_NAME_COL).eq(lit(svc)))?;
        }

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
        let trace_spans = build_span_tree(flat_spans);

        Ok(trace_spans)
    }

    /// Resolve attribute filter strings (`"key:value"`) to matching trace_id hex strings
    /// via `search_blob LIKE '%key:value%'` on the spans table.
    ///
    /// Returns an empty vec when no filters are provided.
    #[instrument(skip_all)]
    pub async fn get_trace_ids_matching_attributes(
        &self,
        attribute_filters: &[String],
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
    ) -> Result<Vec<String>, TraceEngineError> {
        if attribute_filters.is_empty() {
            return Ok(Vec::new());
        }

        let mut builder =
            TraceQueryBuilder::set_table(self.ctx.clone(), SPAN_TABLE_NAME).await?;
        builder = builder.select_columns(&[TRACE_ID_COL, START_TIME_COL, SEARCH_BLOB_COL])?;

        // Time predicates FIRST for partition pruning
        if let Some(start) = start_time {
            builder =
                builder.add_filter(col(START_TIME_COL).gt_eq(lit(start.to_rfc3339())))?;
        }
        if let Some(end) = end_time {
            builder = builder.add_filter(col(START_TIME_COL).lt(lit(end.to_rfc3339())))?;
        }

        // OR-match each "key:value" filter against search_blob (match_all is always false)
        let mut attr_expr: Option<Expr> = None;
        for filter in attribute_filters {
            let pattern = format!("%{}%", filter.replace('\'', "''"));
            let cond = col(SEARCH_BLOB_COL).like(lit(pattern));
            attr_expr = Some(match attr_expr {
                None => cond,
                Some(existing) => existing.or(cond),
            });
        }
        if let Some(expr) = attr_expr {
            builder = builder.add_filter(expr)?;
        }

        let batches = builder.execute().await?;

        let mut trace_ids = Vec::new();
        for batch in &batches {
            let col_arr = batch
                .column_by_name(TRACE_ID_COL)
                .and_then(|c| c.as_any().downcast_ref::<BinaryArray>())
                .ok_or_else(|| {
                    TraceEngineError::BatchConversion("trace_id not BinaryArray".into())
                })?;
            for i in 0..batch.num_rows() {
                let hex = hex::encode(col_arr.value(i));
                if !trace_ids.contains(&hex) {
                    trace_ids.push(hex);
                }
            }
        }

        Ok(trace_ids)
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
        entity_trace_ids: Option<&[Vec<u8>]>,
    ) -> Result<Vec<TraceMetricBucket>, TraceEngineError> {
        const VALID_INTERVALS: &[&str] =
            &["second", "minute", "hour", "day", "week", "month", "year"];
        if !VALID_INTERVALS.contains(&bucket_interval) {
            return Err(TraceEngineError::UnsupportedOperation(format!(
                "Invalid bucket_interval '{}'. Must be one of: {}",
                bucket_interval,
                VALID_INTERVALS.join(", ")
            )));
        }

        let start_rfc = start_time.to_rfc3339();
        let end_rfc = end_time.to_rfc3339();

        // matching_traces CTE — only emitted when attribute_filters is non-empty
        let (matching_traces_cte, attr_trace_clause) = match attribute_filters {
            Some(filters) if !filters.is_empty() => {
                let clauses: Vec<String> = filters
                    .iter()
                    .map(|f| format!("search_blob LIKE '%{}%'", f.replace('\'', "''")))
                    .collect();
                let cte = format!(
                    "matching_traces AS (\
                        SELECT DISTINCT trace_id FROM {table} \
                        WHERE start_time >= '{start}' AND start_time < '{end}' \
                        AND ({attr_cond})\
                    ),",
                    table = SPAN_TABLE_NAME,
                    start = start_rfc,
                    end = end_rfc,
                    attr_cond = clauses.join(" OR "),
                );
                let clause =
                    "AND trace_id IN (SELECT trace_id FROM matching_traces)".to_string();
                (cte, clause)
            }
            _ => (String::new(), String::new()),
        };

        // entity trace_ids IN filter
        let entity_filter_clause = match entity_trace_ids {
            Some(ids) if !ids.is_empty() => {
                let hex_list: Vec<String> =
                    ids.iter().map(|b| format!("X'{}'", hex::encode(b))).collect();
                format!("AND trace_id IN ({})", hex_list.join(", "))
            }
            _ => String::new(),
        };

        // Service filter on root span (parent_span_id IS NULL) — matches Postgres logic
        let service_filter_clause = match service_name {
            Some(svc) => format!(
                "root_service = '{}'",
                svc.replace('\'', "''")
            ),
            None => "TRUE".to_string(),
        };

        // CTE-based query: trace-level duration = MAX(end_time) - MIN(start_time) in µs / 1000.
        // trace_level aggregates per-trace; service_filtered computes duration from the
        // already-aggregated trace_start/trace_end to avoid duplicate aggregate expressions
        // in the same SELECT (which DataFusion rejects).
        let sql = format!(
            "WITH {matching_traces_cte}\
            trace_level AS (\
                SELECT \
                    s.trace_id, \
                    MIN(s.start_time) AS trace_start, \
                    MAX(s.end_time)   AS trace_end, \
                    MAX(CASE WHEN s.parent_span_id IS NULL \
                             THEN CAST(s.service_name AS VARCHAR) END) AS root_service, \
                    MAX(s.status_code) AS status_code \
                FROM {table} s \
                WHERE s.start_time >= '{start}' AND s.start_time < '{end}' \
                {entity_filter} \
                {attr_trace} \
                GROUP BY s.trace_id \
            ), \
            service_filtered AS (\
                SELECT \
                    trace_id, \
                    trace_start, \
                    root_service, \
                    status_code, \
                    (CAST(trace_end AS BIGINT) - CAST(trace_start AS BIGINT)) / 1000 \
                        AS duration_ms \
                FROM trace_level \
                WHERE {svc_filter} \
                AND trace_end IS NOT NULL \
            ), \
            bucketed AS (\
                SELECT \
                    DATE_TRUNC('{interval}', trace_start) AS bucket_start, \
                    duration_ms, \
                    status_code \
                FROM service_filtered \
            ) \
            SELECT \
                bucket_start, \
                COUNT(*) AS trace_count, \
                AVG(CAST(duration_ms AS DOUBLE)) AS avg_duration_ms, \
                approx_percentile_cont(CAST(duration_ms AS DOUBLE), 0.50) AS p50_duration_ms, \
                approx_percentile_cont(CAST(duration_ms AS DOUBLE), 0.95) AS p95_duration_ms, \
                approx_percentile_cont(CAST(duration_ms AS DOUBLE), 0.99) AS p99_duration_ms, \
                AVG(CASE WHEN status_code = 2 THEN 1.0 ELSE 0.0 END) AS error_rate \
            FROM bucketed \
            GROUP BY bucket_start \
            ORDER BY bucket_start ASC",
            matching_traces_cte = matching_traces_cte,
            table = SPAN_TABLE_NAME,
            start = start_rfc,
            end = end_rfc,
            entity_filter = entity_filter_clause,
            attr_trace = attr_trace_clause,
            svc_filter = service_filter_clause,
            interval = bucket_interval,
        );

        let df = self
            .ctx
            .sql(&sql)
            .await
            .map_err(TraceEngineError::DatafusionError)?;

        let batches = df
            .collect()
            .await
            .map_err(TraceEngineError::DatafusionError)?;

        let mut metrics = Vec::new();
        for batch in &batches {
            let schema = batch.schema();

            // DATE_TRUNC may return various Timestamp sub-types depending on DataFusion version.
            // Cast to Int64 (microseconds since epoch) for uniform handling.
            let raw_bucket = batch.column(schema.index_of("bucket_start").unwrap());
            let bucket_i64 = arrow::compute::cast(raw_bucket, &arrow::datatypes::DataType::Int64)
                .map_err(|e| TraceEngineError::BatchConversion(format!("bucket_start cast: {}", e)))?;
            let bucket_col = bucket_i64
                .as_any()
                .downcast_ref::<Int64Array>()
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
                let bucket_start = Utc
                    .timestamp_opt(micros / 1_000_000, ((micros % 1_000_000) * 1_000) as u32)
                    .unwrap();

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

        Ok(metrics)
    }
}
