pub mod genai;
pub mod sql;
pub use genai::{
    extract_gen_ai_span, GenAiAgentActivity, GenAiMetricsRequest, GenAiModelUsage,
    GenAiOperationBreakdown, GenAiSpanFilters, GenAiSpanRecord, GenAiTokenBucket,
    GenAiToolActivity, GEN_AI_AGENT_ID, GEN_AI_AGENT_NAME, GEN_AI_CONVERSATION_ID,
    GEN_AI_ERROR_TYPE, GEN_AI_OPERATION_NAME, GEN_AI_OUTPUT_TYPE, GEN_AI_PROVIDER_NAME,
    GEN_AI_REQUEST_MAX_TOKENS, GEN_AI_REQUEST_MODEL, GEN_AI_REQUEST_TEMPERATURE,
    GEN_AI_REQUEST_TOP_P, GEN_AI_RESPONSE_FINISH_REASONS, GEN_AI_RESPONSE_ID,
    GEN_AI_RESPONSE_MODEL, GEN_AI_TOOL_CALL_ID, GEN_AI_TOOL_NAME, GEN_AI_TOOL_TYPE,
    GEN_AI_USAGE_CACHE_CREATION_INPUT_TOKENS, GEN_AI_USAGE_CACHE_READ_INPUT_TOKENS,
    GEN_AI_USAGE_INPUT_TOKENS, GEN_AI_USAGE_OUTPUT_TOKENS, OPENAI_API_TYPE, OPENAI_SERVICE_TIER,
};

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
use std::collections::HashMap;
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
pub const SCOUTER_EVAL_SCENARIO_ID_ATTR: &str = "scouter.eval.scenario_id";
pub const SCOUTER_QUEUE_RECORD: &str = "scouter.queue.record";
pub const SCOUTER_QUEUE_EVENT: &str = "scouter.queue.event";
pub const SCOUTER_ENTITY: &str = "scouter.entity";

// patterns for identifying baggage and tags
pub const BAGGAGE_PATTERN: &str = "baggage.";
pub const BAGGAGE_TAG_PATTERN: &str = concat!("baggage", ".", "scouter.tracing.tag", ".");
pub const TAG_PATTERN: &str = concat!("scouter.tracing.tag", ".");

type SpanAttributes = (Vec<Attribute>, Vec<TraceBaggageRecord>, Vec<TagRecord>);

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq, Hash)]
pub struct ScouterEntityAttribute {
    pub uid: String,
    pub r#type: String,
    pub space: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
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

    pub fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, RecordError> {
        let bytes = hex::decode(hex)?;
        if bytes.len() == 8 {
            Ok(bytes)
        } else {
            Err(RecordError::SliceError(format!(
                "Invalid hex string length: expected 16 or 8 bytes, got {}",
                bytes.len()
            )))
        }
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
#[pyclass(from_py_object)]
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
#[pyclass(from_py_object)]
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
#[pyclass(from_py_object)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct TraceBaggageRecord {
    #[pyo3(get)]
    pub created_at: DateTime<Utc>,
    pub trace_id: TraceId,
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

    #[getter]
    pub fn get_trace_id(&self) -> String {
        self.trace_id.to_hex()
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
                        trace_id: *trace_id,
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
                    trace_id: *trace_id,
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
        trace_id: &TraceId,
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
                trace_id: *trace_id,
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
#[pyclass(from_py_object)]
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
#[pyclass(from_py_object)]
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
#[pyclass(from_py_object)]
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
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[pyclass(from_py_object)]
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
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[pyclass(from_py_object)]
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

/// Convert a flat list of `TraceSpanRecord`s into a tree-enriched list of `TraceSpan`s.
///
/// Groups records by `trace_id`, performs DFS to compute `depth`, `path`,
/// `root_span_id`, and `span_order` for each span.
pub fn build_trace_spans(records: Vec<TraceSpanRecord>) -> Vec<sql::TraceSpan> {
    if records.is_empty() {
        return Vec::new();
    }

    // Group by trace_id (hex)
    let mut groups: HashMap<String, Vec<&TraceSpanRecord>> = HashMap::new();
    for record in &records {
        groups
            .entry(record.trace_id.to_hex())
            .or_default()
            .push(record);
    }

    let mut all_spans = Vec::with_capacity(records.len());
    let mut global_order: i32 = 0;

    for spans in groups.values() {
        // Build parent→children index (using span_id bytes as key)
        let mut children: HashMap<[u8; 8], Vec<usize>> = HashMap::new();
        let mut root_indices: Vec<usize> = Vec::new();

        for (i, span) in spans.iter().enumerate() {
            if let Some(pid) = &span.parent_span_id {
                children.entry(*pid.as_bytes()).or_default().push(i);
            } else {
                root_indices.push(i);
            }
        }

        // Sort roots by start_time for determinism
        root_indices.sort_by_key(|&i| spans[i].start_time);

        // Determine root_span_id
        let root_span_id_hex = if let Some(&first_root) = root_indices.first() {
            spans[first_root].span_id.to_hex()
        } else {
            // All spans have parents (orphans) — use first span's span_id as fallback
            spans[0].span_id.to_hex()
        };

        // DFS traversal (iterative)
        let pre_dfs_len = all_spans.len();
        dfs_assign_records(
            &root_indices,
            spans,
            &children,
            &root_span_id_hex,
            &mut all_spans,
            &mut global_order,
        );

        // Attach orphan spans (parent not found in this trace group)
        // Only look at spans added by THIS group's DFS to avoid cross-group collisions
        let visited: HashSet<[u8; 8]> = all_spans[pre_dfs_len..]
            .iter()
            .filter_map(|s| {
                let bytes = SpanId::hex_to_bytes(&s.span_id).ok()?;
                let arr: [u8; 8] = bytes.try_into().ok()?;
                Some(arr)
            })
            .collect();

        for span in spans {
            if !visited.contains(span.span_id.as_bytes()) {
                let span_id_hex = span.span_id.to_hex();
                all_spans.push(record_to_trace_span(
                    span,
                    &span_id_hex,
                    &root_span_id_hex,
                    0,
                    vec![span_id_hex.clone()],
                    global_order,
                ));
                global_order += 1;
            }
        }
    }

    all_spans
}

/// Iterative DFS traversal to assign depth, path, and span_order to trace spans.
fn dfs_assign_records(
    root_indices: &[usize],
    spans: &[&TraceSpanRecord],
    children: &HashMap<[u8; 8], Vec<usize>>,
    root_span_id_hex: &str,
    result: &mut Vec<sql::TraceSpan>,
    span_order: &mut i32,
) {
    // Stack entries: (span_index, depth, path_so_far)
    let mut stack: Vec<(usize, i32, Vec<String>)> = Vec::new();
    let mut visited: HashSet<usize> = HashSet::new();

    // Push roots in reverse so the first root is processed first
    for &idx in root_indices.iter().rev() {
        stack.push((idx, 0, Vec::new()));
    }

    while let Some((idx, depth, path_so_far)) = stack.pop() {
        if !visited.insert(idx) {
            continue; // cycle detected — skip
        }
        let span = spans[idx];
        let span_id_hex = span.span_id.to_hex();

        let mut path = path_so_far;
        path.push(span_id_hex.clone());

        result.push(record_to_trace_span(
            span,
            &span_id_hex,
            root_span_id_hex,
            depth,
            path.clone(),
            *span_order,
        ));
        *span_order += 1;

        // Push children in reverse start_time order so earliest is processed first
        if let Some(child_indices) = children.get(span.span_id.as_bytes()) {
            let mut sorted = child_indices.clone();
            sorted.sort_by_key(|&i| spans[i].start_time);
            for &ci in sorted.iter().rev() {
                stack.push((ci, depth + 1, path.clone()));
            }
        }
    }
}

fn record_to_trace_span(
    record: &TraceSpanRecord,
    span_id_hex: &str,
    root_span_id_hex: &str,
    depth: i32,
    path: Vec<String>,
    span_order: i32,
) -> sql::TraceSpan {
    let input = match &record.input {
        Value::Null => None,
        v => Some(v.clone()),
    };
    let output = match &record.output {
        Value::Null => None,
        v => Some(v.clone()),
    };

    sql::TraceSpan {
        trace_id: record.trace_id.to_hex(),
        span_id: span_id_hex.to_string(),
        parent_span_id: record.parent_span_id.as_ref().map(|id| id.to_hex()),
        span_name: record.span_name.clone(),
        span_kind: Some(record.span_kind.clone()),
        start_time: record.start_time,
        end_time: record.end_time,
        duration_ms: record.duration_ms,
        status_code: record.status_code,
        status_message: Some(record.status_message.clone()),
        attributes: record.attributes.clone(),
        events: record.events.clone(),
        links: record.links.clone(),
        depth,
        path,
        root_span_id: root_span_id_hex.to_string(),
        service_name: record.service_name.clone(),
        span_order,
        input,
        output,
    }
}

/// Lightweight trace summary record written to the Delta Lake `trace_summaries` table.
///
/// Produced by converting a `TraceAggregator` (in `scouter_sql`) after the in-memory
/// aggregation phase. Entity tags are written separately to Postgres and are not included here.
#[derive(Clone, Debug)]
pub struct TraceSummaryRecord {
    pub trace_id: TraceId,
    pub service_name: String,
    pub scope_name: String,
    pub scope_version: String,
    pub root_operation: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub status_code: i32,
    pub status_message: String,
    pub span_count: i64,
    pub error_count: i64,
    pub resource_attributes: Vec<Attribute>,
    /// Entity UIDs associated with this trace (from `scouter.entity` attributes).
    pub entity_ids: Vec<String>,
    /// Queue record UIDs associated with this trace (from `scouter.queue.record` attributes).
    pub queue_ids: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_span_record(
        trace_id: [u8; 16],
        span_id: [u8; 8],
        parent_span_id: Option<[u8; 8]>,
        name: &str,
        start_ms: i64,
    ) -> TraceSpanRecord {
        TraceSpanRecord {
            trace_id: TraceId::from_bytes(trace_id),
            span_id: SpanId::from_bytes(span_id),
            parent_span_id: parent_span_id.map(SpanId::from_bytes),
            span_name: name.to_string(),
            start_time: chrono::DateTime::from_timestamp_millis(start_ms).unwrap(),
            end_time: chrono::DateTime::from_timestamp_millis(start_ms + 100).unwrap(),
            duration_ms: 100,
            ..Default::default()
        }
    }

    #[test]
    fn build_trace_spans_empty() {
        let result = build_trace_spans(vec![]);
        assert!(result.is_empty());
    }

    #[test]
    fn build_trace_spans_simple_tree() {
        let tid = [0u8; 16];
        let root_sid = [1u8; 8];
        let child_sid = [2u8; 8];

        let records = vec![
            make_span_record(tid, root_sid, None, "root", 1000),
            make_span_record(tid, child_sid, Some(root_sid), "child", 1050),
        ];

        let spans = build_trace_spans(records);
        assert_eq!(spans.len(), 2);

        // Root span
        let root = spans.iter().find(|s| s.span_name == "root").unwrap();
        assert_eq!(root.depth, 0);
        assert_eq!(root.span_order, 0);
        assert!(root.parent_span_id.is_none());
        assert_eq!(root.path.len(), 1);

        // Child span
        let child = spans.iter().find(|s| s.span_name == "child").unwrap();
        assert_eq!(child.depth, 1);
        assert_eq!(child.span_order, 1);
        assert!(child.parent_span_id.is_some());
        assert_eq!(child.path.len(), 2);
        assert_eq!(child.root_span_id, root.span_id);
    }

    #[test]
    fn build_trace_spans_orphan_spans() {
        let tid = [0u8; 16];
        let orphan_sid = [3u8; 8];
        // Parent doesn't exist in the batch
        let missing_parent = [99u8; 8];

        let records = vec![make_span_record(
            tid,
            orphan_sid,
            Some(missing_parent),
            "orphan",
            1000,
        )];

        let spans = build_trace_spans(records);
        assert_eq!(spans.len(), 1);

        let orphan = &spans[0];
        assert_eq!(orphan.span_name, "orphan");
        assert_eq!(orphan.depth, 0);
    }

    #[test]
    fn build_trace_spans_multiple_traces() {
        let tid1 = [1u8; 16];
        let tid2 = [2u8; 16];

        let records = vec![
            make_span_record(tid1, [10u8; 8], None, "trace1_root", 1000),
            make_span_record(tid2, [20u8; 8], None, "trace2_root", 2000),
            make_span_record(tid1, [11u8; 8], Some([10u8; 8]), "trace1_child", 1050),
        ];

        let spans = build_trace_spans(records);
        assert_eq!(spans.len(), 3);

        // Check that trace_ids are correct
        let t1_spans: Vec<_> = spans
            .iter()
            .filter(|s| s.trace_id == TraceId::from_bytes(tid1).to_hex())
            .collect();
        assert_eq!(t1_spans.len(), 2);

        let t2_spans: Vec<_> = spans
            .iter()
            .filter(|s| s.trace_id == TraceId::from_bytes(tid2).to_hex())
            .collect();
        assert_eq!(t2_spans.len(), 1);
    }

    #[test]
    fn build_trace_spans_deep_tree() {
        let tid = [0u8; 16];
        let root_sid = [1u8; 8];
        let child_sid = [2u8; 8];
        let grandchild_sid = [3u8; 8];

        let records = vec![
            make_span_record(tid, root_sid, None, "root", 1000),
            make_span_record(tid, child_sid, Some(root_sid), "child", 1050),
            make_span_record(tid, grandchild_sid, Some(child_sid), "grandchild", 1100),
        ];

        let spans = build_trace_spans(records);
        assert_eq!(spans.len(), 3);

        let grandchild = spans.iter().find(|s| s.span_name == "grandchild").unwrap();
        assert_eq!(grandchild.depth, 2);
        assert_eq!(grandchild.path.len(), 3); // root → child → grandchild
    }

    #[test]
    fn build_trace_spans_cross_group_collision() {
        // Two different traces where spans happen to share the same span_id bytes.
        // The visited set must be scoped per group to avoid cross-group collisions.
        let tid1 = [1u8; 16];
        let tid2 = [2u8; 16];
        let shared_sid = [42u8; 8]; // Same span_id in both traces

        let records = vec![
            make_span_record(tid1, shared_sid, None, "trace1_root", 1000),
            make_span_record(tid2, shared_sid, None, "trace2_root", 2000),
        ];

        let spans = build_trace_spans(records);
        // Both spans must appear — the cross-group visited set bug would drop the second
        assert_eq!(spans.len(), 2);

        let names: HashSet<&str> = spans.iter().map(|s| s.span_name.as_str()).collect();
        assert!(names.contains("trace1_root"));
        assert!(names.contains("trace2_root"));
    }

    #[test]
    fn build_trace_spans_input_output_mapping() {
        let tid = [0u8; 16];
        let records = vec![TraceSpanRecord {
            trace_id: TraceId::from_bytes(tid),
            span_id: SpanId::from_bytes([1u8; 8]),
            parent_span_id: None,
            span_name: "test".to_string(),
            input: serde_json::json!({"key": "value"}),
            output: Value::Null,
            ..Default::default()
        }];

        let spans = build_trace_spans(records);
        assert_eq!(spans.len(), 1);
        assert!(spans[0].input.is_some());
        assert!(spans[0].output.is_none()); // Null → None
    }

    #[test]
    fn build_trace_spans_cycle_does_not_loop() {
        // Construct a cycle: A → B → A (via parent_span_id pointing back).
        // The DFS visited guard must prevent infinite traversal.
        let tid = [0u8; 16];
        let span_a = [1u8; 8];
        let span_b = [2u8; 8];

        // A is the root (no parent), B claims A as parent,
        // but we also add A as a child of B in the children map by
        // making A's parent_span_id point to B. This creates a cycle.
        let records = vec![
            make_span_record(tid, span_a, Some(span_b), "A", 1000),
            make_span_record(tid, span_b, Some(span_a), "B", 1050),
        ];

        // Both are orphans (neither has a true root), so both become synthetic roots.
        // The key test: this terminates without hanging.
        let spans = build_trace_spans(records);
        assert_eq!(spans.len(), 2, "Both spans should appear exactly once");
    }
}
