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
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::fmt;

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
pub const SPAN_ERROR: &str = "span.error";
pub const EXCEPTION_TRACEBACK: &str = "exception.traceback";
pub const SCOUTER_QUEUE_RECORD: &str = "scouter.queue.record";
pub const SCOUTER_QUEUE_EVENT: &str = "scouter.queue.event";

// patterns for identifying baggage and tags
pub const BAGGAGE_PATTERN: &str = "baggage.";
pub const BAGGAGE_TAG_PATTERN: &str = concat!("baggage", ".", "scouter.tracing.tag", ".");
pub const TAG_PATTERN: &str = concat!("scouter.tracing.tag", ".");

type SpanAttributes = (Vec<Attribute>, Vec<TraceBaggageRecord>, Vec<TagRecord>);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct TraceId([u8; 16]);

impl TraceId {
    pub fn from_hex(hex: &str) -> Result<Self, hex::FromHexError> {
        let mut bytes = [0u8; 16];
        hex::decode_to_slice(hex, &mut bytes)?;
        Ok(Self(bytes))
    }

    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub fn from_slice(slice: &[u8]) -> Result<Self, RecordError> {
        if slice.len() != 16 {
            return Err(RecordError::SliceError(format!(
                "Invalid trace_id length: expected 16 bytes, got {}",
                slice.len()
            )));
        }
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(slice);
        Ok(Self(bytes))
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    pub fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, RecordError> {
        let bytes = hex::decode(hex)?;
        // validate length for trace_id (16 bytes) or span_id (8 bytes)
        if bytes.len() == 16 {
            Ok(bytes)
        } else {
            Err(RecordError::SliceError(format!(
                "Invalid hex string length: expected 16 or 8 bytes, got {}",
                bytes.len()
            )))
        }
    }
}

impl fmt::Display for TraceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl Serialize for TraceId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for TraceId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let hex = String::deserialize(deserializer)?;
        TraceId::from_hex(&hex).map_err(serde::de::Error::custom)
    }
}

#[cfg(feature = "server")]
impl<'r> sqlx::Decode<'r, sqlx::Postgres> for TraceId {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let bytes = <&[u8] as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        if bytes.len() != 16 {
            return Err("TraceId must be exactly 16 bytes".into());
        }
        let mut array = [0u8; 16];
        array.copy_from_slice(bytes);
        Ok(TraceId(array))
    }
}

#[cfg(feature = "server")]
impl sqlx::Type<sqlx::Postgres> for TraceId {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <Vec<u8> as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

#[cfg(feature = "server")]
impl sqlx::Encode<'_, sqlx::Postgres> for TraceId {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <&[u8] as sqlx::Encode<sqlx::Postgres>>::encode(&self.0[..], buf)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct SpanId([u8; 8]);

impl SpanId {
    pub fn from_hex(hex: &str) -> Result<Self, hex::FromHexError> {
        let mut bytes = [0u8; 8];
        hex::decode_to_slice(hex, &mut bytes)?;
        Ok(Self(bytes))
    }

    pub fn from_bytes(bytes: [u8; 8]) -> Self {
        Self(bytes)
    }

    pub fn from_slice(slice: &[u8]) -> Result<Self, RecordError> {
        if slice.len() != 8 {
            return Err(RecordError::SliceError(format!(
                "Invalid trace_id length: expected 8 bytes, got {}",
                slice.len()
            )));
        }
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(slice);
        Ok(Self(bytes))
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    pub fn as_bytes(&self) -> &[u8; 8] {
        &self.0
    }
}

impl fmt::Display for SpanId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

impl Serialize for SpanId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for SpanId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let hex = String::deserialize(deserializer)?;
        SpanId::from_hex(&hex).map_err(serde::de::Error::custom)
    }
}

#[cfg(feature = "server")]
impl<'r> sqlx::Decode<'r, sqlx::Postgres> for SpanId {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let bytes = <&[u8] as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        if bytes.len() != 8 {
            return Err("SpanId must be exactly 8 bytes".into());
        }
        let mut array = [0u8; 8];
        array.copy_from_slice(bytes);
        Ok(SpanId(array))
    }
}

#[cfg(feature = "server")]
impl sqlx::Type<sqlx::Postgres> for SpanId {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <Vec<u8> as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

#[cfg(feature = "server")]
impl sqlx::Encode<'_, sqlx::Postgres> for SpanId {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <&[u8] as sqlx::Encode<sqlx::Postgres>>::encode(&self.0[..], buf)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[pyclass]
pub struct TraceRecord {
    #[pyo3(get)]
    pub created_at: DateTime<Utc>,
    pub trace_id: TraceId,
    #[pyo3(get)]
    pub service_name: String,
    #[pyo3(get)]
    pub scope_name: String,
    #[pyo3(get)]
    pub scope_version: Option<String>,
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
    pub root_span_id: SpanId,
    #[pyo3(get)]
    pub span_count: i32,
    #[pyo3(get)]
    pub tags: Vec<Tag>,
    #[pyo3(get)]
    pub process_attributes: Vec<Attribute>,
}

#[pymethods]
impl TraceRecord {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }

    #[getter]
    pub fn get_trace_id(&self) -> String {
        self.trace_id.to_hex()
    }

    #[getter]
    pub fn get_root_span_id(&self) -> String {
        self.root_span_id.to_hex()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[pyclass]
pub struct TraceSpanRecord {
    #[pyo3(get)]
    pub created_at: chrono::DateTime<Utc>,

    // core identifiers
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub parent_span_id: Option<SpanId>,

    // W3C Trace Context fields
    #[pyo3(get)]
    pub flags: i32,
    #[pyo3(get)]
    pub trace_state: String,

    // instrumentation
    #[pyo3(get)]
    pub scope_name: String,
    #[pyo3(get)]
    pub scope_version: Option<String>,

    // Span metadata
    #[pyo3(get)]
    pub span_name: String,
    #[pyo3(get)]
    pub span_kind: String,

    // Temporal data
    #[pyo3(get)]
    pub start_time: chrono::DateTime<Utc>,
    #[pyo3(get)]
    pub end_time: chrono::DateTime<Utc>,
    #[pyo3(get)]
    pub duration_ms: i64,

    // Status
    #[pyo3(get)]
    pub status_code: i32,
    #[pyo3(get)]
    pub status_message: String,

    // Semi-structured data
    #[pyo3(get)]
    pub attributes: Vec<Attribute>,
    #[pyo3(get)]
    pub events: Vec<SpanEvent>,
    #[pyo3(get)]
    pub links: Vec<SpanLink>,

    // Scouter-specific fields
    #[pyo3(get)]
    pub label: Option<String>,
    pub input: Value,
    pub output: Value,

    // Service reference (denormalized for query performance)
    #[pyo3(get)]
    pub service_name: String,
    #[pyo3(get)]
    pub resource_attributes: Vec<Attribute>,
}

#[pymethods]
impl TraceSpanRecord {
    #[getter]
    pub fn get_trace_id(&self) -> String {
        self.trace_id.to_hex()
    }

    #[getter]
    pub fn get_span_id(&self) -> String {
        self.span_id.to_hex()
    }

    #[getter]
    pub fn get_parent_span_id(&self) -> Option<String> {
        self.parent_span_id.as_ref().map(|id| id.to_hex())
    }

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
    Vec<TraceSpanRecord>,
    Vec<TraceBaggageRecord>,
    Vec<TagRecord>,
);

pub trait TraceRecordExt {
    fn keyvalue_to_json_array<T: Serialize>(attributes: &Vec<T>) -> Result<Value, RecordError> {
        Ok(serde_json::to_value(attributes).unwrap_or(Value::Array(vec![])))
    }

    fn process_attributes(
        trace_id: &TraceId,
        span_attributes: &[KeyValue],
        scope: &str,
        created_at: DateTime<Utc>,
    ) -> Result<SpanAttributes, RecordError> {
        let mut cleaned_attributes = Vec::with_capacity(span_attributes.len());
        let mut baggage_records = Vec::new();
        let mut tags = Vec::new();

        let trace_id_hex = trace_id.to_hex();
        let scope_owned = scope.to_string();

        for kv in span_attributes {
            let key = &kv.key;

            // Check if this is a baggage-prefixed tag
            if let Some(tag_key) = key.strip_prefix(BAGGAGE_TAG_PATTERN) {
                if !tag_key.is_empty() {
                    // tag values are stored as strings for tag table
                    let string_value = match &kv.value {
                        Some(v) => Self::otel_value_to_string(v),
                        None => "null".to_string(),
                    };

                    // Extract as a tag
                    tags.push(TagRecord::from_trace(
                        trace_id,
                        tag_key.to_string(),
                        string_value.clone(),
                    ));

                    // Store cleaned attribute with stripped key
                    cleaned_attributes.push(Attribute {
                        key: tag_key.to_string(),
                        value: Value::String(string_value.clone()),
                    });

                    // Also extract as baggage since it has baggage prefix
                    baggage_records.push(TraceBaggageRecord {
                        created_at,
                        trace_id: trace_id_hex.clone(),
                        scope: scope_owned.clone(),
                        key: format!("{}.{}", SCOUTER_TAG_PREFIX, tag_key),
                        value: string_value,
                    });
                } else {
                    tracing::warn!(
                        attribute_key = %key,
                        "Skipping baggage tag with empty key after prefix removal"
                    );
                }
            }
            // Check for non-baggage tags
            else if let Some(tag_key) = key.strip_prefix(TAG_PATTERN) {
                // tag values are stored as strings for tag table
                if !tag_key.is_empty() {
                    let string_value = match &kv.value {
                        Some(v) => Self::otel_value_to_string(v),
                        None => "null".to_string(),
                    };

                    tags.push(TagRecord::from_trace(
                        trace_id,
                        tag_key.to_string(),
                        string_value.clone(),
                    ));

                    cleaned_attributes.push(Attribute {
                        key: tag_key.to_string(),
                        value: Value::String(string_value.clone()),
                    });
                } else {
                    tracing::warn!(
                        attribute_key = %key,
                        "Skipping tag with empty key after prefix removal"
                    );
                }
            }
            // Check for regular baggage (not tags)
            else if key.starts_with(BAGGAGE_PATTERN) {
                let clean_key = key
                    .strip_prefix(BAGGAGE_PATTERN)
                    .unwrap_or(key)
                    .trim()
                    .to_string();

                let string_value = match &kv.value {
                    Some(v) => Self::otel_value_to_string(v),
                    None => "null".to_string(),
                };

                baggage_records.push(TraceBaggageRecord {
                    created_at,
                    trace_id: trace_id_hex.clone(),
                    scope: scope_owned.clone(),
                    key: clean_key,
                    value: string_value,
                });
            }
            // Regular attribute
            else {
                let value = match &kv.value {
                    Some(v) => otel_value_to_serde_value(v),
                    None => Value::Null,
                };

                cleaned_attributes.push(Attribute {
                    key: key.clone(),
                    value,
                });
            }
        }

        Ok((cleaned_attributes, baggage_records, tags))
    }

    fn otel_value_to_string(value: &AnyValue) -> String {
        match &value.value {
            Some(opentelemetry_proto::tonic::common::v1::any_value::Value::StringValue(s)) => {
                s.clone()
            }
            Some(opentelemetry_proto::tonic::common::v1::any_value::Value::IntValue(i)) => {
                i.to_string()
            }
            Some(opentelemetry_proto::tonic::common::v1::any_value::Value::DoubleValue(d)) => {
                d.to_string()
            }
            Some(opentelemetry_proto::tonic::common::v1::any_value::Value::BoolValue(b)) => {
                b.to_string()
            }
            Some(opentelemetry_proto::tonic::common::v1::any_value::Value::ArrayValue(_))
            | Some(opentelemetry_proto::tonic::common::v1::any_value::Value::KvlistValue(_)) => {
                let serde_val = otel_value_to_serde_value(value);
                serde_json::to_string(&serde_val).unwrap_or_else(|_| format!("{:?}", value))
            }
            _ => "null".to_string(),
        }
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
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct TraceServerRecord {
    pub request: ExportTraceServiceRequest,
}

impl TraceRecordExt for TraceServerRecord {}

impl TraceServerRecord {
    /// Extract InstrumentationScope name and version from ScopeSpan
    fn get_scope_info(
        scope_span: &opentelemetry_proto::tonic::trace::v1::ScopeSpans,
    ) -> (String, Option<String>) {
        let scope_name = scope_span
            .scope
            .as_ref()
            .map(|s| s.name.clone())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "unknown".to_string());

        let scope_version = scope_span.scope.as_ref().and_then(|s| {
            if s.version.is_empty() {
                None
            } else {
                Some(s.version.clone())
            }
        });

        (scope_name, scope_version)
    }

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

    pub fn to_records(self) -> Result<TraceRecords, RecordError> {
        let resource_spans = self.request.resource_spans;

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

        let mut span_records: Vec<TraceSpanRecord> = Vec::with_capacity(estimated_capacity);
        let mut baggage_records: Vec<TraceBaggageRecord> = Vec::new();
        let mut tags: HashSet<TagRecord> = HashSet::new();

        for resource_span in resource_spans {
            // process metadata only once per resource span
            let service_name =
                Self::get_service_name_from_resource(&resource_span.resource, "unknown");
            let resource_attributes = Attribute::from_resources(&resource_span.resource);

            for scope_span in &resource_span.scope_spans {
                let (scope_name, scope_version) = Self::get_scope_info(scope_span);

                for span in &scope_span.spans {
                    // Core identifiers
                    let trace_id = TraceId::from_slice(span.trace_id.as_slice())?;
                    let span_id = SpanId::from_slice(span.span_id.as_slice())?;
                    let parent_span_id = if !span.parent_span_id.is_empty() {
                        Some(SpanId::from_slice(span.parent_span_id.as_slice())?)
                    } else {
                        None
                    };

                    let (start_time, end_time, duration_ms) =
                        Self::extract_time(span.start_time_unix_nano, span.end_time_unix_nano);

                    let (cleaned_attributes, span_baggage, span_tags) = Self::process_attributes(
                        &trace_id,
                        &span.attributes,
                        &scope_name,
                        start_time,
                    )?;

                    // Add to collections
                    baggage_records.extend(span_baggage);
                    tags.extend(span_tags);

                    let (input, output) = Self::extract_input_output(&cleaned_attributes);

                    // SpanRecord for insert
                    span_records.push(TraceSpanRecord {
                        created_at: start_time,
                        trace_id,
                        span_id,
                        parent_span_id,
                        flags: span.flags as i32,
                        trace_state: span.trace_state.clone(),
                        scope_name: scope_name.clone(),
                        scope_version: scope_version.clone(),
                        span_name: span.name.clone(),
                        span_kind: Self::span_kind_to_string(span.kind),
                        start_time,
                        end_time,
                        duration_ms,
                        status_code: span.status.as_ref().map(|s| s.code).unwrap_or(0),
                        status_message: span
                            .status
                            .as_ref()
                            .map(|s| s.message.clone())
                            .unwrap_or_default(),
                        attributes: cleaned_attributes,
                        events: Self::events_to_json_array(&span.events)?,
                        links: Self::links_to_json_array(&span.links)?,
                        label: None,
                        input,
                        output,
                        service_name: service_name.clone(),
                        resource_attributes: resource_attributes.clone(),
                    });
                }
            }
        }

        let tag_records: Vec<TagRecord> = tags.into_iter().collect();
        Ok((span_records, baggage_records, tag_records))
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

    fn from_resources(
        resource: &Option<opentelemetry_proto::tonic::resource::v1::Resource>,
    ) -> Vec<Attribute> {
        match resource {
            Some(res) => res
                .attributes
                .iter()
                .map(|kv| Attribute::from_otel_value(kv.key.clone(), kv.value.as_ref().unwrap()))
                .collect(),
            None => vec![],
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

#[derive(Clone, Debug, Serialize, Deserialize, Eq, Hash, PartialEq)]
#[pyclass]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct TagRecord {
    #[pyo3(get)]
    pub entity_type: String,
    #[pyo3(get)]
    pub entity_id: String,
    #[pyo3(get)]
    pub key: String,
    #[pyo3(get)]
    pub value: String,
}

impl TagRecord {
    /// Create a tag record from a TraceId
    pub fn from_trace(trace_id: &TraceId, key: String, value: String) -> Self {
        Self {
            entity_type: "trace".to_string(),
            entity_id: trace_id.to_hex(),
            key,
            value,
        }
    }
}

#[pymethods]
impl TagRecord {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}
