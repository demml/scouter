//! Zero-copy view over Arrow RecordBatch for TraceSpan data (flat, no hierarchy fields).
//!
//! Hierarchy fields (depth, span_order, path, root_span_id) are NOT stored in Delta Lake —
//! they are computed at query time by `build_span_tree()` in `queries.rs`.

use arrow::array::*;
use chrono::{DateTime, TimeZone, Utc};
use scouter_types::{Attribute, SpanId, TraceId};
use scouter_types::{SpanEvent, SpanLink};
use serde::Serialize;
use std::sync::Arc;
use tracing::{error, instrument};

pub fn extract_attributes_from_map(
    array: &StructArray,
    idx: usize,
    column_name: &str,
) -> Vec<Attribute> {
    let attr_col = array.column_by_name(column_name);

    if attr_col.is_none() {
        return Vec::new();
    }

    let map_array = attr_col
        .and_then(|col| col.as_any().downcast_ref::<MapArray>())
        .expect("attributes should be MapArray");

    if map_array.is_null(idx) {
        return Vec::new();
    }

    let struct_array = map_array.value(idx);
    let keys = struct_array.column(0).as_string::<i32>();
    let values = struct_array.column(1).as_string::<i32>();

    (0..struct_array.len())
        .map(|i| Attribute {
            key: keys.value(i).to_string(),
            value: serde_json::from_str(values.value(i)).unwrap_or(serde_json::Value::Null),
        })
        .collect()
}

/// Zero-copy view of a batch of trace spans backed by Arrow arrays.
///
/// Hierarchy fields are absent (they are computed at query time).
/// Use `TraceQueries::get_trace_spans()` for the full `TraceSpan` type with hierarchy populated.
#[derive(Clone)]
pub struct TraceSpanBatch {
    trace_ids: Arc<BinaryArray>,
    span_ids: Arc<BinaryArray>,
    parent_span_ids: Arc<BinaryArray>,
    flags: Arc<Int32Array>,
    trace_states: Arc<StringArray>,
    scope_names: Arc<StringArray>,
    scope_versions: Arc<StringArray>,
    span_names: Arc<StringArray>,
    service_names: Arc<StringArray>,
    span_kinds: Arc<StringArray>,
    start_times: Arc<TimestampMicrosecondArray>,
    end_times: Arc<TimestampMicrosecondArray>,
    durations: Arc<Int64Array>,
    status_codes: Arc<Int32Array>,
    status_messages: Arc<StringArray>,
    labels: Arc<StringArray>,
    attributes: Arc<MapArray>,
    events: Arc<ListArray>,
    links: Arc<ListArray>,
    inputs: Arc<StringArray>,
    outputs: Arc<StringArray>,

    len: usize,
}

impl TraceSpanBatch {
    /// Create a zero-copy view from a RecordBatch (new schema without hierarchy fields).
    #[instrument(skip_all)]
    pub fn from_record_batch(batch: &RecordBatch) -> Result<Self, arrow::error::ArrowError> {
        let schema = batch.schema();

        macro_rules! get_col {
            ($name:expr, $type:ty) => {{
                let idx = schema.index_of($name).inspect_err(|_| {
                    error!("Column '{}' not found in batch schema", $name);
                })?;
                let array = batch.column(idx);
                Arc::new(
                    array
                        .as_any()
                        .downcast_ref::<$type>()
                        .ok_or_else(|| {
                            error!(
                                "Column {} is not of expected type {}",
                                $name,
                                std::any::type_name::<$type>()
                            );
                            arrow::error::ArrowError::CastError(format!(
                                "Column {} is not {}",
                                $name,
                                std::any::type_name::<$type>()
                            ))
                        })?
                        .clone(),
                )
            }};
        }

        Ok(TraceSpanBatch {
            trace_ids: get_col!("trace_id", BinaryArray),
            span_ids: get_col!("span_id", BinaryArray),
            parent_span_ids: get_col!("parent_span_id", BinaryArray),
            flags: get_col!("flags", Int32Array),
            trace_states: get_col!("trace_state", StringArray),
            scope_names: get_col!("scope_name", StringArray),
            scope_versions: get_col!("scope_version", StringArray),
            span_names: get_col!("span_name", StringArray),
            service_names: get_col!("service_name", StringArray),
            span_kinds: get_col!("span_kind", StringArray),
            start_times: get_col!("start_time", TimestampMicrosecondArray),
            end_times: get_col!("end_time", TimestampMicrosecondArray),
            durations: get_col!("duration_ms", Int64Array),
            status_codes: get_col!("status_code", Int32Array),
            status_messages: get_col!("status_message", StringArray),
            labels: get_col!("label", StringArray),
            attributes: get_col!("attributes", MapArray),
            events: get_col!("events", ListArray),
            links: get_col!("links", ListArray),
            inputs: get_col!("input", StringArray),
            outputs: get_col!("output", StringArray),
            len: batch.num_rows(),
        })
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn get(&self, idx: usize) -> Option<TraceSpanView<'_>> {
        if idx >= self.len {
            return None;
        }
        Some(TraceSpanView { batch: self, idx })
    }

    pub fn iter(&self) -> TraceSpanIterator<'_> {
        TraceSpanIterator {
            batch: self,
            idx: 0,
        }
    }
}

/// Zero-copy view of a single span (no hierarchy fields).
#[derive(Clone, Copy)]
pub struct TraceSpanView<'a> {
    batch: &'a TraceSpanBatch,
    idx: usize,
}

impl<'a> TraceSpanView<'a> {
    pub fn trace_id_bytes(&self) -> &[u8; 16] {
        let bytes = self.batch.trace_ids.value(self.idx);
        bytes.try_into().expect("Trace ID should be 16 bytes")
    }

    pub fn trace_id_hex(&self) -> String {
        TraceId::from_bytes(*self.trace_id_bytes()).to_hex()
    }

    pub fn span_id_bytes(&self) -> &[u8; 8] {
        let bytes = self.batch.span_ids.value(self.idx);
        bytes.try_into().expect("Span ID should be 8 bytes")
    }

    pub fn span_id_hex(&self) -> String {
        SpanId::from_bytes(*self.span_id_bytes()).to_hex()
    }

    pub fn parent_span_id_bytes(&self) -> Option<&[u8; 8]> {
        if self.batch.parent_span_ids.is_null(self.idx) {
            None
        } else {
            let bytes = self.batch.parent_span_ids.value(self.idx);
            Some(bytes.try_into().expect("Parent Span ID should be 8 bytes"))
        }
    }

    pub fn parent_span_id_hex(&self) -> Option<String> {
        self.parent_span_id_bytes()
            .map(|bytes| SpanId::from_bytes(*bytes).to_hex())
    }

    pub fn flags(&self) -> i32 {
        self.batch.flags.value(self.idx)
    }

    pub fn trace_state(&self) -> &str {
        self.batch.trace_states.value(self.idx)
    }

    pub fn scope_name(&self) -> &str {
        self.batch.scope_names.value(self.idx)
    }

    pub fn scope_version(&self) -> Option<&str> {
        if self.batch.scope_versions.is_null(self.idx) {
            None
        } else {
            Some(self.batch.scope_versions.value(self.idx))
        }
    }

    pub fn span_name(&self) -> &str {
        self.batch.span_names.value(self.idx)
    }

    pub fn service_name(&self) -> &str {
        self.batch.service_names.value(self.idx)
    }

    pub fn span_kind(&self) -> Option<&str> {
        if self.batch.span_kinds.is_null(self.idx) {
            None
        } else {
            Some(self.batch.span_kinds.value(self.idx))
        }
    }

    pub fn start_time(&self) -> DateTime<Utc> {
        let micros = self.batch.start_times.value(self.idx);
        let secs = micros / 1_000_000;
        let nanos = ((micros % 1_000_000) * 1_000) as u32;
        Utc.timestamp_opt(secs, nanos).unwrap()
    }

    pub fn end_time(&self) -> DateTime<Utc> {
        let micros = self.batch.end_times.value(self.idx);
        let secs = micros / 1_000_000;
        let nanos = ((micros % 1_000_000) * 1_000) as u32;
        Utc.timestamp_opt(secs, nanos).unwrap()
    }

    pub fn duration_ms(&self) -> i64 {
        self.batch.durations.value(self.idx)
    }

    pub fn status_code(&self) -> i32 {
        self.batch.status_codes.value(self.idx)
    }

    pub fn status_message(&self) -> Option<&str> {
        if self.batch.status_messages.is_null(self.idx) {
            None
        } else {
            Some(self.batch.status_messages.value(self.idx))
        }
    }

    pub fn label(&self) -> Option<&str> {
        if self.batch.labels.is_null(self.idx) {
            None
        } else {
            Some(self.batch.labels.value(self.idx))
        }
    }

    pub fn input_json(&self) -> Option<&str> {
        if self.batch.inputs.is_null(self.idx) {
            None
        } else {
            Some(self.batch.inputs.value(self.idx))
        }
    }

    pub fn output_json(&self) -> Option<&str> {
        if self.batch.outputs.is_null(self.idx) {
            None
        } else {
            Some(self.batch.outputs.value(self.idx))
        }
    }

    pub fn attributes(&self) -> Vec<Attribute> {
        if self.batch.attributes.is_null(self.idx) {
            return Vec::new();
        }
        let struct_array = self.batch.attributes.value(self.idx);
        let keys = struct_array.column(0).as_string::<i32>();
        let values = struct_array.column(1).as_string::<i32>();
        (0..struct_array.len())
            .map(|i| Attribute {
                key: keys.value(i).to_string(),
                value: serde_json::from_str(values.value(i)).unwrap_or(serde_json::Value::Null),
            })
            .collect()
    }

    pub fn events(&self) -> Vec<SpanEvent> {
        if self.batch.events.is_null(self.idx) {
            return Vec::new();
        }
        let array = self.batch.events.value(self.idx);
        let event_list = array.as_struct();
        (0..event_list.len())
            .map(|i| SpanEventView::new(event_list, i).into_event())
            .collect()
    }

    pub fn links(&self) -> Vec<SpanLink> {
        if self.batch.links.is_null(self.idx) {
            return Vec::new();
        }
        let link_list = self.batch.links.value(self.idx);
        let struct_array = link_list.as_struct();
        (0..struct_array.len())
            .map(|i| SpanLinkView::new(struct_array, i).into_link())
            .collect()
    }
}

impl<'a> Serialize for TraceSpanView<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("TraceSpanView", 21)?;

        state.serialize_field("trace_id", &self.trace_id_hex())?;
        state.serialize_field("span_id", &self.span_id_hex())?;
        state.serialize_field("parent_span_id", &self.parent_span_id_hex())?;
        state.serialize_field("flags", &self.flags())?;
        state.serialize_field("trace_state", self.trace_state())?;
        state.serialize_field("scope_name", self.scope_name())?;
        state.serialize_field("scope_version", &self.scope_version())?;
        state.serialize_field("span_name", self.span_name())?;
        state.serialize_field("service_name", self.service_name())?;
        state.serialize_field("span_kind", &self.span_kind())?;
        state.serialize_field("start_time", &self.start_time())?;
        state.serialize_field("end_time", &self.end_time())?;
        state.serialize_field("duration_ms", &self.duration_ms())?;
        state.serialize_field("status_code", &self.status_code())?;
        state.serialize_field("status_message", &self.status_message())?;
        state.serialize_field("label", &self.label())?;
        state.serialize_field("input", &self.input_json())?;
        state.serialize_field("output", &self.output_json())?;
        state.serialize_field("attributes", &self.attributes())?;
        state.serialize_field("events", &self.events())?;
        state.serialize_field("links", &self.links())?;

        state.end()
    }
}

pub struct TraceSpanIterator<'a> {
    batch: &'a TraceSpanBatch,
    idx: usize,
}

impl<'a> Iterator for TraceSpanIterator<'a> {
    type Item = TraceSpanView<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.batch.len() {
            return None;
        }
        let view = TraceSpanView {
            batch: self.batch,
            idx: self.idx,
        };
        self.idx += 1;
        Some(view)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.batch.len() - self.idx;
        (remaining, Some(remaining))
    }
}

impl<'a> ExactSizeIterator for TraceSpanIterator<'a> {}

pub struct SpanEventView<'a> {
    array: &'a StructArray,
    idx: usize,
}

impl<'a> SpanEventView<'a> {
    fn new(array: &'a StructArray, idx: usize) -> Self {
        Self { array, idx }
    }

    pub fn timestamp(&self) -> DateTime<Utc> {
        let timestamp_array = self
            .array
            .column_by_name("timestamp")
            .and_then(|col| col.as_any().downcast_ref::<TimestampMicrosecondArray>())
            .expect("timestamp should be TimestampMicrosecondArray");

        let micros = timestamp_array.value(self.idx);
        let secs = micros / 1_000_000;
        let nanos = ((micros % 1_000_000) * 1_000) as u32;
        Utc.timestamp_opt(secs, nanos).unwrap()
    }

    pub fn name(&self) -> &str {
        let name_array = self
            .array
            .column_by_name("name")
            .and_then(|col| col.as_any().downcast_ref::<StringArray>())
            .expect("name should be StringArray");
        name_array.value(self.idx)
    }

    pub fn attributes(&self) -> Vec<Attribute> {
        extract_attributes_from_map(self.array, self.idx, "attributes")
    }

    pub fn dropped_attributes_count(&self) -> u32 {
        let count_array = self
            .array
            .column_by_name("dropped_attributes_count")
            .and_then(|col| col.as_any().downcast_ref::<UInt32Array>())
            .expect("dropped_attributes_count should be UInt32Array");
        count_array.value(self.idx)
    }

    fn into_event(self) -> SpanEvent {
        SpanEvent {
            timestamp: self.timestamp(),
            name: self.name().to_string(),
            attributes: self.attributes(),
            dropped_attributes_count: self.dropped_attributes_count(),
        }
    }
}

pub struct SpanLinkView<'a> {
    array: &'a StructArray,
    idx: usize,
}

impl<'a> SpanLinkView<'a> {
    fn new(array: &'a StructArray, idx: usize) -> Self {
        Self { array, idx }
    }

    pub fn trace_id(&self) -> String {
        let trace_id_array = self
            .array
            .column_by_name("trace_id")
            .map(|col| col.as_fixed_size_binary())
            .expect("trace_id should be FixedSizeBinaryArray");

        let bytes = trace_id_array.value(self.idx);
        let bytes_array: [u8; 16] = bytes.try_into().expect("trace_id should be 16 bytes");
        TraceId::from_bytes(bytes_array).to_hex()
    }

    pub fn span_id(&self) -> String {
        let span_id_array = self
            .array
            .column_by_name("span_id")
            .map(|col| col.as_fixed_size_binary())
            .expect("span_id should be FixedSizeBinaryArray");

        let bytes = span_id_array.value(self.idx);
        let bytes_array: [u8; 8] = bytes.try_into().expect("span_id should be 8 bytes");
        SpanId::from_bytes(bytes_array).to_hex()
    }

    pub fn trace_state(&self) -> &str {
        let trace_state_array = self
            .array
            .column_by_name("trace_state")
            .map(|col| col.as_string::<i32>())
            .expect("trace_state should be StringArray");

        if trace_state_array.is_null(self.idx) {
            ""
        } else {
            trace_state_array.value(self.idx)
        }
    }

    pub fn attributes(&self) -> Vec<Attribute> {
        extract_attributes_from_map(self.array, self.idx, "attributes")
    }

    pub fn dropped_attributes_count(&self) -> u32 {
        let count_array = self
            .array
            .column_by_name("dropped_attributes_count")
            .and_then(|col| col.as_any().downcast_ref::<UInt32Array>())
            .expect("dropped_attributes_count should be UInt32Array");
        count_array.value(self.idx)
    }

    fn into_link(self) -> SpanLink {
        SpanLink {
            trace_id: self.trace_id(),
            span_id: self.span_id(),
            trace_state: self.trace_state().to_string(),
            attributes: self.attributes(),
            dropped_attributes_count: self.dropped_attributes_count(),
        }
    }
}
