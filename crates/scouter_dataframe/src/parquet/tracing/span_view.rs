//! Zero-copy view over Arrow RecordBatch for TraceSpan data
//!
//! This module provides efficient, zero-allocation access to trace span data
//! by holding references directly to Arrow arrays. Allocations only happen
//! during serialization (e.g., hex encoding IDs, JSON serialization).

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

/// Zero-copy view of a trace span backed by Arrow arrays
///
/// Benefits over owned TraceSpan:
/// - No string allocations until serialization
/// - No hex encoding overhead until needed
/// - Direct memory access to Arrow buffers
/// - Multiple spans share same Arrow array backing
///
/// Use case: Query millions of spans, serialize subset to API response
#[derive(Clone)]
pub struct TraceSpanBatch {
    // Hold Arc to keep arrays alive
    trace_ids: Arc<BinaryArray>, // datafusion/deltalake read back binary
    span_ids: Arc<BinaryArray>,
    parent_span_ids: Arc<BinaryArray>,
    root_span_ids: Arc<BinaryArray>,
    span_names: Arc<StringArray>,
    service_names: Arc<StringArray>,
    span_kinds: Arc<StringArray>,
    start_times: Arc<TimestampMicrosecondArray>,
    end_times: Arc<TimestampMicrosecondArray>,
    durations: Arc<Int64Array>,
    status_codes: Arc<Int32Array>,
    status_messages: Arc<StringArray>,
    depths: Arc<Int32Array>,
    span_orders: Arc<Int32Array>,
    paths: Arc<ListArray>,
    attributes: Arc<MapArray>,
    events: Arc<ListArray>,
    links: Arc<ListArray>,
    inputs: Arc<StringArray>,
    outputs: Arc<StringArray>,

    // Number of rows in this batch
    len: usize,
}

impl TraceSpanBatch {
    /// Create a zero-copy view from a RecordBatch
    ///
    /// This doesn't allocate - just holds Arc references to arrays
    #[instrument(skip_all)]
    pub fn from_record_batch(batch: &RecordBatch) -> Result<Self, arrow::error::ArrowError> {
        let schema = batch.schema();

        // Macro to extract typed array with error handling
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
            root_span_ids: get_col!("root_span_id", BinaryArray),
            span_names: get_col!("span_name", StringArray),
            service_names: get_col!("service_name", StringArray),
            span_kinds: get_col!("span_kind", StringArray),
            start_times: get_col!("start_time", TimestampMicrosecondArray),
            end_times: get_col!("end_time", TimestampMicrosecondArray),
            durations: get_col!("duration_ms", Int64Array),
            status_codes: get_col!("status_code", Int32Array),
            status_messages: get_col!("status_message", StringArray),
            depths: get_col!("depth", Int32Array),
            span_orders: get_col!("span_order", Int32Array),
            paths: get_col!("path", ListArray),
            attributes: get_col!("attributes", MapArray),
            events: get_col!("events", ListArray),
            links: get_col!("links", ListArray),
            inputs: get_col!("input", StringArray),
            outputs: get_col!("output", StringArray),
            len: batch.num_rows(),
        })
    }

    /// Number of spans in this batch
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get a view of a single span (zero-copy)
    pub fn get(&self, idx: usize) -> Option<TraceSpanView<'_>> {
        if idx >= self.len {
            return None;
        }

        Some(TraceSpanView { batch: self, idx })
    }

    /// Iterator over all spans in this batch (zero-copy)
    pub fn iter(&self) -> TraceSpanIterator<'_> {
        TraceSpanIterator {
            batch: self,
            idx: 0,
        }
    }
}

/// Zero-copy view of a single span within a batch
///
/// This struct holds no data - just a reference to the batch and an index.
/// All field access is done on-demand without allocation.
#[derive(Clone, Copy)]
pub struct TraceSpanView<'a> {
    batch: &'a TraceSpanBatch,
    idx: usize,
}

impl<'a> TraceSpanView<'a> {
    /// Get trace ID as raw bytes (zero-copy)
    pub fn trace_id_bytes(&self) -> &[u8; 16] {
        let bytes = self.batch.trace_ids.value(self.idx);
        bytes.try_into().expect("Trace ID should be 16 bytes")
    }

    /// Get trace ID as hex string (allocates)
    pub fn trace_id_hex(&self) -> String {
        TraceId::from_bytes(*self.trace_id_bytes()).to_hex()
    }

    /// Get span ID as raw bytes (zero-copy)
    pub fn span_id_bytes(&self) -> &[u8; 8] {
        let bytes = self.batch.span_ids.value(self.idx);
        bytes.try_into().expect("Span ID should be 8 bytes")
    }

    /// Get span ID as hex string (allocates)
    pub fn span_id_hex(&self) -> String {
        SpanId::from_bytes(*self.span_id_bytes()).to_hex()
    }

    /// Get parent span ID as raw bytes (zero-copy)
    pub fn parent_span_id_bytes(&self) -> Option<&[u8; 8]> {
        if self.batch.parent_span_ids.is_null(self.idx) {
            None
        } else {
            let bytes = self.batch.parent_span_ids.value(self.idx);
            Some(bytes.try_into().expect("Parent Span ID should be 8 bytes"))
        }
    }

    /// Get parent span ID as hex string (allocates)
    pub fn parent_span_id_hex(&self) -> Option<String> {
        self.parent_span_id_bytes()
            .map(|bytes| SpanId::from_bytes(*bytes).to_hex())
    }

    /// Get root span ID as raw bytes (zero-copy)
    pub fn root_span_id_bytes(&self) -> &[u8; 8] {
        let bytes = self.batch.root_span_ids.value(self.idx);
        bytes.try_into().expect("Root Span ID should be 8 bytes")
    }

    /// Get root span ID as hex string (allocates)
    pub fn root_span_id_hex(&self) -> String {
        SpanId::from_bytes(*self.root_span_id_bytes()).to_hex()
    }

    /// Get span name as string slice (zero-copy)
    pub fn span_name(&self) -> &str {
        self.batch.span_names.value(self.idx)
    }

    /// Get service name as string slice (zero-copy)
    pub fn service_name(&self) -> &str {
        self.batch.service_names.value(self.idx)
    }

    /// Get span kind as string slice (zero-copy)
    pub fn span_kind(&self) -> Option<&str> {
        if self.batch.span_kinds.is_null(self.idx) {
            None
        } else {
            Some(self.batch.span_kinds.value(self.idx))
        }
    }

    /// Get start time as DateTime<Utc>
    pub fn start_time(&self) -> DateTime<Utc> {
        let micros = self.batch.start_times.value(self.idx);
        let secs = micros / 1_000_000;
        let nanos = ((micros % 1_000_000) * 1_000) as u32;
        Utc.timestamp_opt(secs, nanos).unwrap()
    }

    /// Get end time as DateTime<Utc>
    pub fn end_time(&self) -> DateTime<Utc> {
        let micros = self.batch.end_times.value(self.idx);
        let secs = micros / 1_000_000;
        let nanos = ((micros % 1_000_000) * 1_000) as u32;
        Utc.timestamp_opt(secs, nanos).unwrap()
    }

    /// Get duration in milliseconds
    pub fn duration_ms(&self) -> i64 {
        self.batch.durations.value(self.idx)
    }

    /// Get status code
    pub fn status_code(&self) -> i32 {
        self.batch.status_codes.value(self.idx)
    }

    /// Get status message (zero-copy)
    pub fn status_message(&self) -> Option<&str> {
        if self.batch.status_messages.is_null(self.idx) {
            None
        } else {
            Some(self.batch.status_messages.value(self.idx))
        }
    }

    /// Get span depth in tree
    pub fn depth(&self) -> i32 {
        self.batch.depths.value(self.idx)
    }

    /// Get span order (for tree traversal)
    pub fn span_order(&self) -> i32 {
        self.batch.span_orders.value(self.idx)
    }

    /// Get path as list of span IDs (returns iterator to avoid allocation)
    pub fn path_iter(&self) -> impl Iterator<Item = &'a str> {
        PathIterator::new(self.batch, self.idx)
    }

    /// Get input JSON (zero-copy string slice, parse on-demand)
    pub fn input_json(&self) -> Option<&str> {
        if self.batch.inputs.is_null(self.idx) {
            None
        } else {
            Some(self.batch.inputs.value(self.idx))
        }
    }

    /// Get output JSON (zero-copy string slice, parse on-demand)
    pub fn output_json(&self) -> Option<&str> {
        if self.batch.outputs.is_null(self.idx) {
            None
        } else {
            Some(self.batch.outputs.value(self.idx))
        }
    }

    /// Extract attributes as key-value pairs
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

    /// Extract events as structured data
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

    /// Extract links as structured data
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

/// Implement Serialize to convert directly from Arrow to JSON
/// This is where allocations happen - only during serialization
impl<'a> Serialize for TraceSpanView<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("TraceSpan", 19)?;

        // Allocate hex strings only during serialization
        state.serialize_field("trace_id", &self.trace_id_hex())?;
        state.serialize_field("span_id", &self.span_id_hex())?;
        state.serialize_field("parent_span_id", &self.parent_span_id_hex())?;
        state.serialize_field("root_span_id", &self.root_span_id_hex())?;

        // Zero-copy string slices
        state.serialize_field("span_name", self.span_name())?;
        state.serialize_field("service_name", self.service_name())?;
        state.serialize_field("span_kind", &self.span_kind())?;

        // Times
        state.serialize_field("start_time", &self.start_time())?;
        state.serialize_field("end_time", &self.end_time())?;
        state.serialize_field("duration_ms", &self.duration_ms())?;

        // Status
        state.serialize_field("status_code", &self.status_code())?;
        state.serialize_field("status_message", &self.status_message())?;

        // Hierarchy
        state.serialize_field("depth", &self.depth())?;
        state.serialize_field("span_order", &self.span_order())?;

        // Path (collect into Vec for serialization)
        state.serialize_field("path", &self.path_iter().collect::<Vec<_>>())?;

        // JSON fields (parse on-demand if needed, or serialize as raw string)
        state.serialize_field("input", &self.input_json())?;
        state.serialize_field("output", &self.output_json())?;

        state.serialize_field("attributes", &self.attributes())?;
        state.serialize_field("events", &self.events())?;
        state.serialize_field("links", &self.links())?;

        state.end()
    }
}

/// Iterator over spans in a batch
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

/// Iterator over path elements
///
/// This iterator maintains a reference to the TraceSpanBatch, which ensures
/// the underlying Arrow arrays remain valid for the lifetime 'a. This allows
/// us to return string slices without unsafe code.
enum PathIterator<'a> {
    Empty,
    NonEmpty {
        batch: &'a TraceSpanBatch,
        span_idx: usize,
        path_idx: usize,
        path_len: usize,
    },
}

impl<'a> PathIterator<'a> {
    fn new(batch: &'a TraceSpanBatch, span_idx: usize) -> Self {
        // Check if this span has a path
        if batch.paths.is_null(span_idx) {
            return PathIterator::Empty;
        }

        // Get the length of the path list for this span
        let path_len = batch.paths.value_length(span_idx) as usize;

        PathIterator::NonEmpty {
            batch,
            span_idx,
            path_idx: 0,
            path_len,
        }
    }
}

impl<'a> Iterator for PathIterator<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            PathIterator::Empty => None,
            PathIterator::NonEmpty {
                batch,
                span_idx,
                path_idx,
                path_len,
            } => {
                if *path_idx >= *path_len {
                    return None;
                }

                // Get the offset for this span's list in the flattened values array
                let offset = batch.paths.value_offsets()[*span_idx] as usize;

                // Get the underlying StringArray from the ListArray
                let string_array = batch
                    .paths
                    .values()
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .expect("Path values should be StringArray");

                // Calculate the actual index in the flattened array
                let actual_idx = offset + *path_idx;

                // Get the string value - this is safe because:
                // 1. We hold a reference to the batch for lifetime 'a
                // 2. The batch holds Arc<ListArray> which keeps the data alive
                // 3. The returned &str is valid for as long as the batch reference is valid
                let value = string_array.value(actual_idx);
                *path_idx += 1;

                Some(value)
            }
        }
    }
}

/// Zero-copy view of a span event
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

/// Zero-copy view of a span link
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
