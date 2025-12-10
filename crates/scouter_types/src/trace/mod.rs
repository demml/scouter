pub mod sql;

use crate::error::RecordError;
use crate::otel_value_to_serde_value;
use crate::PyHelperFuncs;
use crate::{json_to_pyobject, json_to_pyobject_value};
use chrono::DateTime;
use chrono::Utc;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::tonic::common::v1::AnyValue;
use opentelemetry_proto::tonic::common::v1::KeyValue;
use opentelemetry_proto::tonic::trace::v1::span::Event;
use opentelemetry_proto::tonic::trace::v1::span::Link;
use opentelemetry_proto::tonic::trace::v1::span::SpanKind;
use opentelemetry_proto::tonic::trace::v1::Span;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::{max, min};
use std::collections::HashMap;

pub const FUNCTION_TYPE: &str = "function.type";
pub const FUNCTION_STREAMING: &str = "function.streaming";
pub const FUNCTION_NAME: &str = "function.name";
pub const FUNCTION_MODULE: &str = "function.module";
pub const FUNCTION_QUALNAME: &str = "function.qualname";
pub const SCOUTER_TRACING_INPUT: &str = "scouter.tracing.input";
pub const SCOUTER_TRACING_OUTPUT: &str = "scouter.tracing.output";
pub const SCOUTER_TRACING_LABEL: &str = "scouter.tracing.label";
pub const SERVICE_NAME: &str = "service.name";
pub const SCOUTER_TAG_PREFIX: &str = "scouter.tracing.tag";
pub const BAGGAGE_PREFIX: &str = "baggage";
pub const TRACE_START_TIME_KEY: &str = "scouter.tracing.start_time";
pub const SCOUTER_SCOPE: &str = "scouter.scope";
pub const SCOUTER_SCOPE_DEFAULT: &str = concat!("scouter.tracer.", env!("CARGO_PKG_VERSION"));

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[pyclass]
pub struct TraceRecord {
    #[pyo3(get)]
    pub created_at: DateTime<Utc>,
    #[pyo3(get)]
    pub trace_id: String,
    #[pyo3(get)]
    pub service_name: String,
    #[pyo3(get)]
    pub scope: String,
    #[pyo3(get)]
    pub trace_state: String,
    #[pyo3(get)]
    pub start_time: chrono::DateTime<Utc>,
    #[pyo3(get)]
    pub end_time: chrono::DateTime<Utc>,
    #[pyo3(get)]
    pub duration_ms: i64,
    #[pyo3(get)]
    pub status_code: i32,
    #[pyo3(get)]
    pub status_message: String,
    #[pyo3(get)]
    pub root_span_id: String,
    #[pyo3(get)]
    pub span_count: i32,
    #[pyo3(get)]
    pub tags: Vec<Tag>,
}

#[pymethods]
impl TraceRecord {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

impl TraceRecord {
    /// Merges data from another TraceRecord belonging to the same trace.
    /// This is crucial for updating a trace record as more spans arrive.
    pub fn merge(&mut self, other: &TraceRecord) {
        // 1. Update the overall trace time bounds
        self.start_time = min(self.start_time, other.start_time);
        self.end_time = max(self.end_time, other.end_time);

        // 2. Recalculate duration based on new time bounds
        if self.end_time > self.start_time {
            self.duration_ms = (self.end_time - self.start_time).num_milliseconds();
        } else {
            // Handle edge case where end_time may not be set yet (duration = 0)
            self.duration_ms = 0;
        }

        if self.status_code != 2 && other.status_code == 2 {
            self.status_code = 2;
        }

        self.span_count += other.span_count;

        let mut existing_tag_keys: std::collections::HashSet<String> =
            self.tags.iter().map(|t| t.key.clone()).collect();

        for tag in &other.tags {
            if !existing_tag_keys.contains(&tag.key) {
                self.tags.push(tag.clone());
                existing_tag_keys.insert(tag.key.clone());
            }
        }
    }
}

#[derive(Hash, Eq, PartialEq, Clone)]
struct TraceKey {
    created_at: chrono::DateTime<chrono::Utc>, // Or whatever your created_at type is
    trace_id: String,
    scope: String,
}

pub fn deduplicate_and_merge_traces(raw_traces: Vec<TraceRecord>) -> Vec<TraceRecord> {
    let mut merged_traces: HashMap<TraceKey, TraceRecord> = HashMap::new();

    for trace in raw_traces {
        let key = TraceKey {
            created_at: trace.created_at,
            trace_id: trace.trace_id.clone(),
            scope: trace.scope.clone(),
        };

        merged_traces
            .entry(key)
            .and_modify(|existing_trace| {
                existing_trace.merge(&trace);
            })
            .or_insert(trace);
    }

    merged_traces.into_values().collect()
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[pyclass]
pub struct TraceSpanRecord {
    #[pyo3(get)]
    pub created_at: chrono::DateTime<Utc>,
    #[pyo3(get)]
    pub span_id: String,
    #[pyo3(get)]
    pub trace_id: String,
    #[pyo3(get)]
    pub parent_span_id: Option<String>,
    #[pyo3(get)]
    pub scope: String,
    #[pyo3(get)]
    pub span_name: String,
    #[pyo3(get)]
    pub span_kind: String,
    #[pyo3(get)]
    pub start_time: chrono::DateTime<Utc>,
    #[pyo3(get)]
    pub end_time: chrono::DateTime<Utc>,
    #[pyo3(get)]
    pub duration_ms: i64,
    #[pyo3(get)]
    pub status_code: i32,
    #[pyo3(get)]
    pub status_message: String,
    #[pyo3(get)]
    pub attributes: Vec<Attribute>,
    #[pyo3(get)]
    pub events: Vec<SpanEvent>,
    #[pyo3(get)]
    pub links: Vec<SpanLink>,
    #[pyo3(get)]
    pub label: Option<String>,
    pub input: Value,
    pub output: Value,
    #[pyo3(get)]
    pub service_name: String,
}

#[pymethods]
impl TraceSpanRecord {
    #[getter]
    pub fn get_input<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyDict>, RecordError> {
        let dict = PyDict::new(py);
        match &self.input {
            Value::Null => {}
            _ => {
                json_to_pyobject(py, &self.input, &dict)?;
            }
        }
        Ok(dict)
    }

    #[getter]
    pub fn get_output<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyDict>, RecordError> {
        let dict = PyDict::new(py);
        match &self.output {
            Value::Null => {}
            _ => {
                json_to_pyobject(py, &self.output, &dict)?;
            }
        }
        Ok(dict)
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[pyclass]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct TraceBaggageRecord {
    #[pyo3(get)]
    pub created_at: DateTime<Utc>,
    #[pyo3(get)]
    pub trace_id: String,
    #[pyo3(get)]
    pub scope: String,
    #[pyo3(get)]
    pub key: String,
    #[pyo3(get)]
    pub value: String,
}

#[pymethods]
impl TraceBaggageRecord {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

pub type TraceRecords = (
    Vec<TraceRecord>,
    Vec<TraceSpanRecord>,
    Vec<TraceBaggageRecord>,
);

pub trait TraceRecordExt {
    fn keyvalue_to_json_array<T: Serialize>(attributes: &Vec<T>) -> Result<Value, RecordError> {
        Ok(serde_json::to_value(attributes).unwrap_or(Value::Array(vec![])))
    }

    fn attributes_to_json_array(attributes: &[KeyValue]) -> Result<Vec<Attribute>, RecordError> {
        attributes
            .iter()
            .map(|kv| {
                let value = match &kv.value {
                    Some(v) => otel_value_to_serde_value(v),
                    None => Value::Null,
                };
                Ok(Attribute {
                    key: kv.key.clone(),
                    value,
                })
            })
            .collect()
    }

    fn events_to_json_array(attributes: &[Event]) -> Result<Vec<SpanEvent>, RecordError> {
        attributes
            .iter()
            .map(|kv| {
                let attributes = Self::attributes_to_json_array(&kv.attributes)?;
                Ok(SpanEvent {
                    name: kv.name.clone(),
                    timestamp: DateTime::<Utc>::from_timestamp_nanos(kv.time_unix_nano as i64),
                    attributes,
                    dropped_attributes_count: kv.dropped_attributes_count,
                })
            })
            .collect()
    }

    fn links_to_json_array(attributes: &[Link]) -> Result<Vec<SpanLink>, RecordError> {
        attributes
            .iter()
            .map(|kv| {
                let attributes = Self::attributes_to_json_array(&kv.attributes)?;
                Ok(SpanLink {
                    trace_id: hex::encode(&kv.trace_id),
                    span_id: hex::encode(&kv.span_id),
                    trace_state: kv.trace_state.clone(),
                    attributes,
                    dropped_attributes_count: kv.dropped_attributes_count,
                })
            })
            .collect()
    }

    //// Extracts scouter tags from OpenTelemetry span attributes.
    ///
    /// Tags are identified by the pattern `baggage.scouter.tracing.tag.<key>` and are
    /// converted to a simplified Tag structure for easier processing and storage
    ///
    /// # Arguments
    /// * `attributes` - Vector of OpenTelemetry attributes to search through
    ///
    /// # Returns
    /// * `Result<Vec<Tag>, RecordError>` - Vector of extracted tags or error
    fn extract_tags(attributes: &[Attribute]) -> Result<Vec<Tag>, RecordError> {
        let pattern = format!("{}.{}.", BAGGAGE_PREFIX, SCOUTER_TAG_PREFIX);

        let tags: Result<Vec<Tag>, RecordError> = attributes
            .iter()
            .filter_map(|attr| {
                // Only process attributes that match our pattern
                attr.key.strip_prefix(&pattern).and_then(|tag_key| {
                    // Skip empty tag keys for data integrity
                    if tag_key.is_empty() {
                        tracing::warn!(
                            attribute_key = %attr.key,
                            "Skipping tag with empty key after prefix removal"
                        );
                        return None;
                    }

                    let value = match &attr.value {
                        Value::String(s) => s.clone(),
                        Value::Number(n) => n.to_string(),
                        Value::Bool(b) => b.to_string(),
                        Value::Null => "null".to_string(),

                        // tags should always be string:string
                        Value::Array(_) | Value::Object(_) => {
                            // For complex types, use compact JSON representation
                            serde_json::to_string(&attr.value)
                                .unwrap_or_else(|_| format!("{:?}", attr.value))
                        }
                    };

                    Some(Ok(Tag {
                        key: tag_key.to_string(),
                        value,
                    }))
                })
            })
            .collect();

        tags
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct TraceServerRecord {
    pub request: ExportTraceServiceRequest,
}

impl TraceRecordExt for TraceServerRecord {}

impl TraceServerRecord {
    /// Safely convert OpenTelemetry timestamps to DateTime<Utc> and calculate duration
    ///
    /// # Arguments
    /// * `start_time` - Start timestamp in nanoseconds since Unix epoch
    /// * `end_time` - End timestamp in nanoseconds since Unix epoch
    ///
    /// # Returns
    /// Tuple of (start_time, end_time, duration_ms) with proper error handling
    fn extract_time(start_time: u64, end_time: u64) -> (DateTime<Utc>, DateTime<Utc>, i64) {
        // Safe timestamp conversion with bounds checking
        let start_dt = Self::safe_timestamp_conversion(start_time);
        let end_dt = Self::safe_timestamp_conversion(end_time);

        // Calculate duration with overflow protection
        let duration_ms = if end_time >= start_time {
            let duration_nanos = end_time.saturating_sub(start_time);
            (duration_nanos / 1_000_000).min(i64::MAX as u64) as i64
        } else {
            tracing::warn!(
                start_time = start_time,
                end_time = end_time,
                "Invalid timestamp order detected in trace span"
            );
            0
        };

        (start_dt, end_dt, duration_ms)
    }

    /// Safely convert u64 nanosecond timestamp to DateTime<Utc>
    fn safe_timestamp_conversion(timestamp_nanos: u64) -> DateTime<Utc> {
        if timestamp_nanos <= i64::MAX as u64 {
            DateTime::from_timestamp_nanos(timestamp_nanos as i64)
        } else {
            let seconds = timestamp_nanos / 1_000_000_000;
            let nanoseconds = (timestamp_nanos % 1_000_000_000) as u32;

            DateTime::from_timestamp(seconds as i64, nanoseconds).unwrap_or_else(|| {
                tracing::warn!(
                    timestamp = timestamp_nanos,
                    seconds = seconds,
                    nanoseconds = nanoseconds,
                    "Failed to convert large timestamp, falling back to current time"
                );
                Utc::now()
            })
        }
    }

    /// Safely convert span kind i32 to string with proper error handling
    fn span_kind_to_string(kind: i32) -> String {
        SpanKind::try_from(kind)
            .map(|sk| {
                sk.as_str_name()
                    .strip_prefix("SPAN_KIND_")
                    .unwrap_or(sk.as_str_name())
            })
            .unwrap_or("UNSPECIFIED")
            .to_string()
    }

    fn extract_input_output(attributes: &[Attribute]) -> (Value, Value) {
        let mut input = Value::Null;
        let mut output = Value::Null;

        for attr in attributes {
            if attr.key == SCOUTER_TRACING_INPUT {
                if let Value::String(s) = &attr.value {
                    input = serde_json::from_str(s).unwrap_or_else(|e| {
                        tracing::warn!(
                            key = SCOUTER_TRACING_INPUT,
                            error = %e,
                            value = s,
                            "Failed to parse input attribute as JSON, falling back to string value."
                        );
                        Value::String(s.clone()) // Or Value::Null
                    });
                }
            } else if attr.key == SCOUTER_TRACING_OUTPUT {
                if let Value::String(s) = &attr.value {
                    output = serde_json::from_str(s)
                        .unwrap_or_else(|e| {
                            tracing::warn!(
                                key = SCOUTER_TRACING_OUTPUT,
                                error = %e,
                                value = s,
                                "Failed to parse output attribute as JSON, falling back to string value."
                            );
                            Value::String(s.clone()) // Or Value::Null
                        });
                }
            }
        }
        (input, output)
    }
    /// Convert to TraceRecord
    #[allow(clippy::too_many_arguments)]
    pub fn convert_to_trace_record(
        &self,
        trace_id: &str,
        span_id: &str,
        span: &Span,
        scope_name: &str,
        attributes: &Vec<Attribute>,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        duration_ms: i64,
        service_name: String,
    ) -> Result<TraceRecord, RecordError> {
        Ok(TraceRecord {
            created_at: Self::get_trace_start_time_attribute(attributes, &start_time),
            trace_id: trace_id.to_string(),
            service_name,
            scope: scope_name.to_string(),
            trace_state: span.trace_state.clone(),
            start_time,
            end_time,
            duration_ms,
            status_code: span.status.as_ref().map(|s| s.code).unwrap_or_else(|| 0),
            status_message: span
                .status
                .as_ref()
                .map(|s| s.message.clone())
                .unwrap_or_default(),
            root_span_id: span_id.to_string(),
            tags: Self::extract_tags(attributes)?,
            span_count: 1,
        })
    }

    /// Filter and extract trace start time attribute from span attributes
    /// This is a global scouter attribute that indicates the trace start time and is set across all spans
    pub fn get_trace_start_time_attribute(
        attributes: &Vec<Attribute>,
        start_time: &DateTime<Utc>,
    ) -> DateTime<Utc> {
        for attr in attributes {
            if attr.key == TRACE_START_TIME_KEY {
                if let Value::String(s) = &attr.value {
                    if let Ok(dt) = s.parse::<chrono::DateTime<chrono::Utc>>() {
                        return dt;
                    }
                }
            }
        }

        tracing::warn!(
            "Trace start time attribute not found or invalid, falling back to span start_time"
        );
        *start_time
    }

    fn get_scope_from_resource(
        resource: &Option<opentelemetry_proto::tonic::resource::v1::Resource>,
        default: &str,
    ) -> String {
        resource
            .as_ref()
            .and_then(|r| r.attributes.iter().find(|attr| attr.key == SCOUTER_SCOPE))
            .and_then(|attr| {
                attr.value.as_ref().and_then(|v| {
                    if let Some(
                        opentelemetry_proto::tonic::common::v1::any_value::Value::StringValue(s),
                    ) = &v.value
                    {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
            })
            .unwrap_or_else(|| default.to_string())
    }

    fn get_service_name_from_resource(
        resource: &Option<opentelemetry_proto::tonic::resource::v1::Resource>,
        default: &str,
    ) -> String {
        resource
            .as_ref()
            .and_then(|r| r.attributes.iter().find(|attr| attr.key == SERVICE_NAME))
            .and_then(|attr| {
                attr.value.as_ref().and_then(|v| {
                    if let Some(
                        opentelemetry_proto::tonic::common::v1::any_value::Value::StringValue(s),
                    ) = &v.value
                    {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
            })
            .unwrap_or_else(|| {
                tracing::warn!(
                    "Service name not found in resource attributes, falling back to default: {}",
                    default
                );
                default.to_string()
            })
    }

    pub fn convert_to_baggage_records(
        trace_id: &str,
        attributes: &Vec<Attribute>,
        scope_name: &str,
    ) -> Vec<TraceBaggageRecord> {
        let baggage_kvs: Vec<(String, String)> = attributes
            .iter()
            .filter_map(|attr| {
                // Only process attributes with baggage prefix
                if attr.key.starts_with(BAGGAGE_PREFIX) {
                    let clean_key = attr
                        .key
                        .strip_prefix(format!("{}.", BAGGAGE_PREFIX).as_str())
                        .map(|stripped| stripped.trim())
                        .unwrap_or(&attr.key)
                        .to_string();

                    // Handle different value types from OpenTelemetry KeyValue
                    let value_string = match &attr.value {
                        Value::String(s) => s.clone(),
                        Value::Number(n) => n.to_string(),
                        Value::Bool(b) => b.to_string(),
                        Value::Null => "null".to_string(),
                        Value::Array(_) | Value::Object(_) => {
                            // For complex types, use compact JSON representation
                            serde_json::to_string(&attr.value)
                                .unwrap_or_else(|_| format!("{:?}", attr.value))
                        }
                    };

                    Some((clean_key, value_string))
                } else {
                    None
                }
            })
            .collect();

        baggage_kvs
            .into_iter()
            .map(|(key, value)| TraceBaggageRecord {
                created_at: Self::get_trace_start_time_attribute(attributes, &Utc::now()),
                trace_id: trace_id.to_string(),
                scope: scope_name.to_string(),
                key,
                value,
            })
            .collect()
    }

    /// Convert to TraceRecord
    #[allow(clippy::too_many_arguments)]
    pub fn convert_to_span_record(
        &self,
        trace_id: &str,
        span_id: &str,
        span: &Span,
        attributes: &Vec<Attribute>,
        scope_name: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        duration_ms: i64,
        service_name: String,
    ) -> Result<TraceSpanRecord, RecordError> {
        // get parent span id (can be empty)
        let parent_span_id = if !span.parent_span_id.is_empty() {
            Some(hex::encode(&span.parent_span_id))
        } else {
            None
        };

        let (input, output) = Self::extract_input_output(attributes);

        Ok(TraceSpanRecord {
            created_at: start_time,
            trace_id: trace_id.to_string(),
            span_id: span_id.to_string(),
            parent_span_id,
            start_time,
            end_time,
            duration_ms,
            service_name,
            scope: scope_name.to_string(),
            span_name: span.name.clone(),
            span_kind: Self::span_kind_to_string(span.kind),
            status_code: span.status.as_ref().map(|s| s.code).unwrap_or_else(|| 0),
            status_message: span
                .status
                .as_ref()
                .map(|s| s.message.clone())
                .unwrap_or_default(),
            attributes: attributes.to_owned(),
            events: Self::events_to_json_array(&span.events)?,
            links: Self::links_to_json_array(&span.links)?,
            label: None,
            input,
            output,
        })
    }

    pub fn to_records(&self) -> Result<TraceRecords, RecordError> {
        let resource_spans = &self.request.resource_spans;

        // Pre-calculate capacity to avoid reallocations
        let estimated_capacity: usize = resource_spans
            .iter()
            .map(|rs| {
                rs.scope_spans
                    .iter()
                    .map(|ss| ss.spans.len())
                    .sum::<usize>()
            })
            .sum();

        let mut trace_records: Vec<TraceRecord> = Vec::with_capacity(estimated_capacity);
        let mut span_records: Vec<TraceSpanRecord> = Vec::with_capacity(estimated_capacity);
        let mut baggage_records: Vec<TraceBaggageRecord> = Vec::new();

        for resource_span in resource_spans {
            let service_name =
                Self::get_service_name_from_resource(&resource_span.resource, "unknown");
            let scope = Self::get_scope_from_resource(&resource_span.resource, "unknown");

            for scope_span in &resource_span.scope_spans {
                for span in &scope_span.spans {
                    let attributes = Self::attributes_to_json_array(&span.attributes)?;
                    let trace_id = hex::encode(&span.trace_id);
                    let span_id = hex::encode(&span.span_id);
                    let service_name = service_name.clone();

                    // no need to recalculate for every record type
                    let (start_time, end_time, duration_ms) =
                        Self::extract_time(span.start_time_unix_nano, span.end_time_unix_nano);

                    // TraceRecord for upsert
                    trace_records.push(self.convert_to_trace_record(
                        &trace_id,
                        &span_id,
                        span,
                        &scope,
                        &attributes,
                        start_time,
                        end_time,
                        duration_ms,
                        service_name.clone(),
                    )?);

                    // SpanRecord for insert
                    span_records.push(self.convert_to_span_record(
                        &trace_id,
                        &span_id,
                        span,
                        &attributes,
                        &scope,
                        start_time,
                        end_time,
                        duration_ms,
                        service_name,
                    )?);

                    // BaggageRecords for insert
                    baggage_records.extend(Self::convert_to_baggage_records(
                        &trace_id,
                        &attributes,
                        &scope,
                    ));
                }
            }
        }

        // sort traces by start_time ascending to ensure deterministic merging (we want later spans to update earlier ones)
        trace_records.sort_by_key(|trace| trace.start_time);
        let mut trace_records = deduplicate_and_merge_traces(trace_records);

        // shrink trace_records to fit after deduplication
        trace_records.shrink_to_fit();
        Ok((trace_records, span_records, baggage_records))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[pyclass]
pub struct Attribute {
    #[pyo3(get)]
    pub key: String,
    pub value: Value,
}

#[pymethods]
impl Attribute {
    #[getter]
    pub fn get_value<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyAny>, RecordError> {
        Ok(json_to_pyobject_value(py, &self.value)?.bind(py).clone())
    }

    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

impl Attribute {
    pub fn from_otel_value(key: String, value: &AnyValue) -> Self {
        Attribute {
            key,
            value: otel_value_to_serde_value(value),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[pyclass]
pub struct SpanEvent {
    #[pyo3(get)]
    pub timestamp: chrono::DateTime<Utc>,
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub attributes: Vec<Attribute>,
    #[pyo3(get)]
    pub dropped_attributes_count: u32,
}

#[pymethods]
impl SpanEvent {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[pyclass]
pub struct SpanLink {
    #[pyo3(get)]
    pub trace_id: String,
    #[pyo3(get)]
    pub span_id: String,
    #[pyo3(get)]
    pub trace_state: String,
    #[pyo3(get)]
    pub attributes: Vec<Attribute>,
    #[pyo3(get)]
    pub dropped_attributes_count: u32,
}

#[pymethods]
impl SpanLink {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[pyclass]
pub struct Tag {
    #[pyo3(get)]
    pub key: String,
    #[pyo3(get)]
    pub value: String,
}

#[pymethods]
impl Tag {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[pyclass]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct TagRecord {
    #[pyo3(get)]
    pub created_at: DateTime<Utc>,
    #[pyo3(get)]
    pub entity_type: String,
    #[pyo3(get)]
    pub entity_id: String,
    #[pyo3(get)]
    pub key: String,
    #[pyo3(get)]
    pub value: String,
}

#[pymethods]
impl TagRecord {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}
