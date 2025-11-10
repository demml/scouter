use crate::error::RecordError;
use crate::PyHelperFuncs;
use crate::Status;
use chrono::DateTime;
use chrono::Utc;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::tonic::common::v1::any_value::Value as ProtoAnyValue;
use opentelemetry_proto::tonic::trace::v1::span::SpanKind;
use opentelemetry_proto::tonic::trace::v1::Span;
use pyo3::prelude::*;
use pyo3::IntoPyObjectExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Display;

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

#[pyclass(eq)]
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub enum RecordType {
    #[default]
    Spc,
    Psi,
    Observability,
    Custom,
    LLMDrift,
    LLMMetric,
    Trace,
}

impl Display for RecordType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecordType::Spc => write!(f, "spc"),
            RecordType::Psi => write!(f, "psi"),
            RecordType::Observability => write!(f, "observability"),
            RecordType::Custom => write!(f, "custom"),
            RecordType::LLMDrift => write!(f, "llm_drift"),
            RecordType::LLMMetric => write!(f, "llm_metric"),
            RecordType::Trace => write!(f, "trace"),
        }
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpcServerRecord {
    #[pyo3(get)]
    pub created_at: chrono::DateTime<Utc>,

    #[pyo3(get)]
    pub space: String,

    #[pyo3(get)]
    pub name: String,

    #[pyo3(get)]
    pub version: String,

    #[pyo3(get)]
    pub feature: String,

    #[pyo3(get)]
    pub value: f64,
}

#[pymethods]
impl SpcServerRecord {
    #[new]
    pub fn new(space: String, name: String, version: String, feature: String, value: f64) -> Self {
        Self {
            created_at: Utc::now(),
            name,
            space,
            version,
            feature,
            value,
        }
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }

    pub fn get_record_type(&self) -> RecordType {
        RecordType::Spc
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__json__(self)
    }

    pub fn to_dict(&self) -> HashMap<String, String> {
        let mut record = HashMap::new();
        record.insert("created_at".to_string(), self.created_at.to_string());
        record.insert("name".to_string(), self.name.clone());
        record.insert("space".to_string(), self.space.clone());
        record.insert("version".to_string(), self.version.clone());
        record.insert("feature".to_string(), self.feature.clone());
        record.insert("value".to_string(), self.value.to_string());
        record
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PsiServerRecord {
    #[pyo3(get)]
    pub created_at: chrono::DateTime<Utc>,

    #[pyo3(get)]
    pub space: String,

    #[pyo3(get)]
    pub name: String,

    #[pyo3(get)]
    pub version: String,

    #[pyo3(get)]
    pub feature: String,

    #[pyo3(get)]
    pub bin_id: usize,

    #[pyo3(get)]
    pub bin_count: usize,
}

#[pymethods]
impl PsiServerRecord {
    #[new]
    pub fn new(
        space: String,
        name: String,
        version: String,
        feature: String,
        bin_id: usize,
        bin_count: usize,
    ) -> Self {
        Self {
            created_at: Utc::now(),
            name,
            space,
            version,
            feature,
            bin_id,
            bin_count,
        }
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }

    pub fn get_record_type(&self) -> RecordType {
        RecordType::Psi
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__json__(self)
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LLMDriftServerRecord {
    #[pyo3(get)]
    pub created_at: chrono::DateTime<Utc>,

    #[pyo3(get)]
    pub space: String,

    #[pyo3(get)]
    pub name: String,

    #[pyo3(get)]
    pub version: String,

    pub prompt: Option<Value>,

    pub context: Value,

    pub status: Status,

    pub id: i64,

    pub uid: String,

    pub score: Value,

    pub updated_at: Option<DateTime<Utc>>,

    pub processing_started_at: Option<DateTime<Utc>>,

    pub processing_ended_at: Option<DateTime<Utc>>,

    pub processing_duration: Option<i32>,
}

#[pymethods]
impl LLMDriftServerRecord {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }

    pub fn get_record_type(&self) -> RecordType {
        RecordType::LLMDrift
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__json__(self)
    }
}

impl LLMDriftServerRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new_rs(
        space: String,
        name: String,
        version: String,
        prompt: Option<Value>,
        context: Value,
        created_at: DateTime<Utc>,
        uid: String,
        score: Value,
    ) -> Self {
        Self {
            created_at,
            space,
            name,
            version,
            prompt,
            context,
            status: Status::Pending,
            id: 0, // This is a placeholder, as the ID will be set by the database
            uid,
            score,
            updated_at: None,
            processing_started_at: None,
            processing_ended_at: None,
            processing_duration: None,
        }
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BoxedLLMDriftServerRecord {
    pub record: Box<LLMDriftServerRecord>,
}

impl BoxedLLMDriftServerRecord {
    pub fn new(record: LLMDriftServerRecord) -> Self {
        Self {
            record: Box::new(record),
        }
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LLMMetricRecord {
    #[pyo3(get)]
    pub record_uid: String,

    #[pyo3(get)]
    pub created_at: chrono::DateTime<Utc>,

    #[pyo3(get)]
    pub space: String,

    #[pyo3(get)]
    pub name: String,

    #[pyo3(get)]
    pub version: String,

    #[pyo3(get)]
    pub metric: String,

    #[pyo3(get)]
    pub value: f64,
}

#[pymethods]
impl LLMMetricRecord {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomMetricServerRecord {
    #[pyo3(get)]
    pub created_at: chrono::DateTime<Utc>,

    #[pyo3(get)]
    pub space: String,

    #[pyo3(get)]
    pub name: String,

    #[pyo3(get)]
    pub version: String,

    #[pyo3(get)]
    pub metric: String,

    #[pyo3(get)]
    pub value: f64,
}

#[pymethods]
impl CustomMetricServerRecord {
    #[new]
    pub fn new(space: String, name: String, version: String, metric: String, value: f64) -> Self {
        Self {
            created_at: chrono::Utc::now(),
            name,
            space,
            version,
            metric: metric.to_lowercase(),
            value,
        }
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__json__(self)
    }

    pub fn get_record_type(&self) -> RecordType {
        RecordType::Custom
    }

    pub fn to_dict(&self) -> HashMap<String, String> {
        let mut record = HashMap::new();
        record.insert("created_at".to_string(), self.created_at.to_string());
        record.insert("name".to_string(), self.name.clone());
        record.insert("space".to_string(), self.space.clone());
        record.insert("version".to_string(), self.version.clone());
        record.insert("metric".to_string(), self.metric.clone());
        record.insert("value".to_string(), self.value.to_string());
        record
    }
}

#[pyclass]
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct LatencyMetrics {
    #[pyo3(get)]
    pub p5: f64,

    #[pyo3(get)]
    pub p25: f64,

    #[pyo3(get)]
    pub p50: f64,

    #[pyo3(get)]
    pub p95: f64,

    #[pyo3(get)]
    pub p99: f64,
}

#[pyclass]
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct RouteMetrics {
    #[pyo3(get)]
    pub route_name: String,

    #[pyo3(get)]
    pub metrics: LatencyMetrics,

    #[pyo3(get)]
    pub request_count: i64,

    #[pyo3(get)]
    pub error_count: i64,

    #[pyo3(get)]
    pub error_latency: f64,

    #[pyo3(get)]
    pub status_codes: HashMap<usize, i64>,
}

#[pyclass]
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ObservabilityMetrics {
    #[pyo3(get)]
    pub space: String,

    #[pyo3(get)]
    pub name: String,

    #[pyo3(get)]
    pub version: String,

    #[pyo3(get)]
    pub request_count: i64,

    #[pyo3(get)]
    pub error_count: i64,

    #[pyo3(get)]
    pub route_metrics: Vec<RouteMetrics>,
}

#[pymethods]
impl ObservabilityMetrics {
    pub fn model_dump_json(&self) -> String {
        // serialize records to a string
        PyHelperFuncs::__json__(self)
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }

    pub fn get_record_type(&self) -> RecordType {
        RecordType::Observability
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ServerRecord {
    Spc(SpcServerRecord),
    Psi(PsiServerRecord),
    Custom(CustomMetricServerRecord),
    Observability(ObservabilityMetrics),
    LLMDrift(BoxedLLMDriftServerRecord),
    LLMMetric(LLMMetricRecord),
}

#[pymethods]
impl ServerRecord {
    #[new]
    pub fn new(record: &Bound<'_, PyAny>) -> Result<Self, RecordError> {
        let record_type = record
            .call_method0("get_record_type")?
            .extract::<RecordType>()?;

        match record_type {
            RecordType::Spc => {
                let spc_record = record.extract::<SpcServerRecord>()?;
                Ok(ServerRecord::Spc(spc_record))
            }
            RecordType::Psi => {
                let psi_record = record.extract::<PsiServerRecord>()?;
                Ok(ServerRecord::Psi(psi_record))
            }
            RecordType::Custom => {
                let custom_record = record.extract::<CustomMetricServerRecord>()?;
                Ok(ServerRecord::Custom(custom_record))
            }
            RecordType::Observability => {
                let observability_record = record.extract::<ObservabilityMetrics>()?;
                Ok(ServerRecord::Observability(observability_record))
            }
            RecordType::LLMDrift => {
                let llm_drift_record = record.extract::<LLMDriftServerRecord>()?;
                Ok(ServerRecord::LLMDrift(BoxedLLMDriftServerRecord::new(
                    llm_drift_record,
                )))
            }

            _ => Err(RecordError::InvalidDriftTypeError),
        }
    }

    #[getter]
    pub fn record(&self, py: Python) -> Result<Py<PyAny>, RecordError> {
        match self {
            ServerRecord::Spc(record) => Ok(record.clone().into_py_any(py)?),
            ServerRecord::Psi(record) => Ok(record.clone().into_py_any(py)?),
            ServerRecord::Custom(record) => Ok(record.clone().into_py_any(py)?),
            ServerRecord::Observability(record) => Ok(record.clone().into_py_any(py)?),
            ServerRecord::LLMDrift(record) => Ok(record.record.clone().into_py_any(py)?),
            ServerRecord::LLMMetric(record) => Ok(record.clone().into_py_any(py)?),
        }
    }

    pub fn space(&self) -> String {
        match self {
            ServerRecord::Spc(record) => record.space.clone(),
            ServerRecord::Psi(record) => record.space.clone(),
            ServerRecord::Custom(record) => record.space.clone(),
            ServerRecord::Observability(record) => record.space.clone(),
            ServerRecord::LLMDrift(record) => record.record.space.clone(),
            ServerRecord::LLMMetric(record) => record.space.clone(),
        }
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        match self {
            ServerRecord::Spc(record) => record.__str__(),
            ServerRecord::Psi(record) => record.__str__(),
            ServerRecord::Custom(record) => record.__str__(),
            ServerRecord::Observability(record) => record.__str__(),
            ServerRecord::LLMDrift(record) => record.record.__str__(),
            ServerRecord::LLMMetric(record) => record.__str__(),
        }
    }

    pub fn get_record_type(&self) -> RecordType {
        match self {
            ServerRecord::Spc(_) => RecordType::Spc,
            ServerRecord::Psi(_) => RecordType::Psi,
            ServerRecord::Custom(_) => RecordType::Custom,
            ServerRecord::Observability(_) => RecordType::Observability,
            ServerRecord::LLMDrift(_) => RecordType::LLMDrift,
            ServerRecord::LLMMetric(_) => RecordType::LLMMetric,
        }
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerRecords {
    #[pyo3(get)]
    pub records: Vec<ServerRecord>,
}

#[pymethods]
impl ServerRecords {
    #[new]
    pub fn new(records: Vec<ServerRecord>) -> Self {
        Self { records }
    }
    pub fn model_dump_json(&self) -> String {
        // serialize records to a string
        PyHelperFuncs::__json__(self)
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }
}

impl ServerRecords {
    pub fn record_type(&self) -> Result<RecordType, RecordError> {
        if let Some(first) = self.records.first() {
            Ok(first.get_record_type())
        } else {
            Err(RecordError::EmptyServerRecordsError)
        }
    }
    // Helper function to load records from bytes. Used by scouter-server consumers
    //
    // # Arguments
    //
    // * `bytes` - A slice of bytes
    pub fn load_from_bytes(bytes: &[u8]) -> Result<Self, RecordError> {
        let records: ServerRecords = serde_json::from_slice(bytes)?;
        Ok(records)
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn unique_dates(&self) -> Result<Vec<DateTime<Utc>>, RecordError> {
        let mut dates = HashSet::new();
        let record_type = self.record_type().unwrap_or(RecordType::Spc);
        for record in &self.records {
            match record {
                ServerRecord::Spc(inner) => {
                    if record_type == RecordType::Spc {
                        dates.insert(inner.created_at);
                    }
                }
                ServerRecord::Psi(inner) => {
                    if record_type == RecordType::Psi {
                        dates.insert(inner.created_at);
                    }
                }
                ServerRecord::Custom(inner) => {
                    if record_type == RecordType::Custom {
                        dates.insert(inner.created_at);
                    }
                }
                _ => {
                    return Err(RecordError::InvalidDriftTypeError);
                }
            }
        }
        let dates: Vec<DateTime<Utc>> = dates.into_iter().collect();

        Ok(dates)
    }
    pub fn space(&self) -> String {
        match self.records.first() {
            Some(record) => record.space(),
            None => "__missing__".to_string(),
        }
    }
}

pub trait ToDriftRecords {
    fn to_spc_drift_records(&self) -> Result<Vec<SpcServerRecord>, RecordError>;
    fn to_observability_drift_records(&self) -> Result<Vec<ObservabilityMetrics>, RecordError>;
    fn to_psi_drift_records(&self) -> Result<Vec<PsiServerRecord>, RecordError>;
    fn to_custom_metric_drift_records(&self) -> Result<Vec<CustomMetricServerRecord>, RecordError>;
    fn to_llm_drift_records(&self) -> Result<Vec<LLMDriftServerRecord>, RecordError>;
    fn to_llm_metric_records(&self) -> Result<Vec<LLMMetricRecord>, RecordError>;
}
impl ToDriftRecords for ServerRecords {
    fn to_spc_drift_records(&self) -> Result<Vec<SpcServerRecord>, RecordError> {
        extract_records(self, |record| match record {
            ServerRecord::Spc(inner) => Some(inner.clone()),
            _ => None,
        })
    }

    fn to_observability_drift_records(&self) -> Result<Vec<ObservabilityMetrics>, RecordError> {
        extract_records(self, |record| match record {
            ServerRecord::Observability(inner) => Some(inner.clone()),
            _ => None,
        })
    }

    fn to_psi_drift_records(&self) -> Result<Vec<PsiServerRecord>, RecordError> {
        extract_records(self, |record| match record {
            ServerRecord::Psi(inner) => Some(inner.clone()),
            _ => None,
        })
    }

    fn to_custom_metric_drift_records(&self) -> Result<Vec<CustomMetricServerRecord>, RecordError> {
        extract_records(self, |record| match record {
            ServerRecord::Custom(inner) => Some(inner.clone()),
            _ => None,
        })
    }

    fn to_llm_drift_records(&self) -> Result<Vec<LLMDriftServerRecord>, RecordError> {
        extract_records(self, |record| match record {
            ServerRecord::LLMDrift(inner) => Some(*inner.record.clone()),
            _ => None,
        })
    }

    fn to_llm_metric_records(&self) -> Result<Vec<LLMMetricRecord>, RecordError> {
        extract_records(self, |record| match record {
            ServerRecord::LLMMetric(inner) => Some(inner.clone()),
            _ => None,
        })
    }
}

// Helper function to extract records of a specific type
fn extract_records<T>(
    server_records: &ServerRecords,
    extractor: impl Fn(&ServerRecord) -> Option<T>,
) -> Result<Vec<T>, RecordError> {
    let mut records = Vec::new();

    for record in &server_records.records {
        if let Some(extracted) = extractor(record) {
            records.push(extracted);
        } else {
            return Err(RecordError::InvalidDriftTypeError);
        }
    }

    Ok(records)
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct TraceRecord {
    pub created_at: DateTime<Utc>,
    pub trace_id: String,
    pub space: String,
    pub name: String,
    pub version: String,
    pub scope: String,
    pub trace_state: String,
    pub start_time: chrono::DateTime<Utc>,
    pub end_time: chrono::DateTime<Utc>,
    pub duration_ms: i64,
    pub status: String,
    pub root_span_id: String,
    pub attributes: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct TraceSpanRecord {
    pub created_at: chrono::DateTime<Utc>,
    pub span_id: String,
    pub trace_id: String,
    pub parent_span_id: Option<String>,
    pub space: String,
    pub name: String,
    pub version: String,
    pub scope: String,
    pub span_name: String,
    pub span_kind: String,
    pub start_time: chrono::DateTime<Utc>,
    pub end_time: chrono::DateTime<Utc>,
    pub duration_ms: i64,
    pub status_code: String,
    pub status_message: String,
    pub attributes: Value,
    pub events: Value,
    pub links: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct TraceBaggageRecord {
    pub created_at: DateTime<Utc>,
    pub trace_id: String,
    pub scope: String,
    pub key: String,
    pub value: String,
    pub space: String,
    pub name: String,
    pub version: String,
}

pub type TraceRecords = (
    Vec<TraceRecord>,
    Vec<TraceSpanRecord>,
    Vec<TraceBaggageRecord>,
);

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct TraceServerRecord {
    pub space: String,
    pub name: String,
    pub version: String,
    pub request: ExportTraceServiceRequest,
}

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

    /// Convert to TraceRecord
    #[allow(clippy::too_many_arguments)]
    pub fn convert_to_trace_record(
        &self,
        span: &Span,
        scope_name: &str,
        scope_attributes: Option<Value>,
        space: &str,
        name: &str,
        version: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        duration_ms: i64,
    ) -> TraceRecord {
        TraceRecord {
            created_at: Self::get_trace_start_time_attribute(span, &start_time),
            trace_id: hex::encode(&span.trace_id),
            space: space.to_owned(),
            name: name.to_owned(),
            version: version.to_owned(),
            scope: scope_name.to_string(),
            trace_state: span.trace_state.clone(),
            start_time,
            end_time,
            duration_ms,
            // More efficient status formatting
            status: match &span.status {
                Some(status) => format!("{:?}", status),
                None => "Unknown".to_string(),
            },
            root_span_id: hex::encode(&span.span_id),
            attributes: scope_attributes.clone(),
        }
    }

    /// Filter and extract trace start time attribute from span attributes
    /// This is a global scouter attribute that indicates the trace start time and is set across all spans
    pub fn get_trace_start_time_attribute(
        span: &Span,
        start_time: &DateTime<Utc>,
    ) -> DateTime<Utc> {
        for attr in &span.attributes {
            if attr.key == TRACE_START_TIME_KEY {
                if let Some(value) = &attr.value {
                    if let Some(ProtoAnyValue::StringValue(s)) = &value.value {
                        if let Ok(dt) = s.parse::<chrono::DateTime<chrono::Utc>>() {
                            return dt;
                        }
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
        span: &Span,
        scope_name: &str,
        space: &str,
        name: &str,
        version: &str,
    ) -> Vec<TraceBaggageRecord> {
        let baggage_kvs: Vec<(String, String)> = span
            .attributes
            .iter()
            .filter_map(|attr| {
                // Only process attributes with baggage prefix
                if attr.key.starts_with(BAGGAGE_PREFIX) {
                    let clean_key = attr
                        .key
                        .strip_prefix(BAGGAGE_PREFIX)
                        .and_then(|stripped| stripped.strip_prefix('.').or(Some(stripped)))
                        .filter(|s| !s.is_empty())
                        .unwrap_or(&attr.key)
                        .to_string();

                    // Handle different value types from OpenTelemetry KeyValue
                    let value_string = match &attr.value {
                        Some(any_value) => {
                            // Convert AnyValue to string representation
                            match &any_value.value {
                                Some(ProtoAnyValue::StringValue(s)) => s.clone(),
                                Some(ProtoAnyValue::IntValue(i)) => i.to_string(),
                                Some(ProtoAnyValue::DoubleValue(d)) => d.to_string(),
                                Some(ProtoAnyValue::BoolValue(b)) => b.to_string(),
                                Some(ProtoAnyValue::BytesValue(bytes)) => {
                                    String::from_utf8_lossy(bytes).to_string()
                                }
                                _ => format!("{:?}", any_value), // Fallback for complex types
                            }
                        }
                        None => String::new(), // Handle missing values gracefully
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
                created_at: Self::get_trace_start_time_attribute(span, &Utc::now()),
                trace_id: hex::encode(&span.trace_id),
                scope: scope_name.to_string(),
                space: space.to_owned(),
                name: name.to_owned(),
                version: version.to_owned(),
                key,
                value,
            })
            .collect()
    }

    /// Convert to TraceRecord
    #[allow(clippy::too_many_arguments)]
    pub fn convert_to_span_record(
        &self,
        span: &Span,
        scope_name: &str,
        space: &str,
        name: &str,
        version: &str,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        duration_ms: i64,
    ) -> TraceSpanRecord {
        // get parent span id (can be empty)
        let parent_span_id = if !span.parent_span_id.is_empty() {
            Some(hex::encode(&span.parent_span_id))
        } else {
            None
        };

        TraceSpanRecord {
            created_at: start_time,
            trace_id: hex::encode(&span.trace_id),
            span_id: hex::encode(&span.span_id),
            parent_span_id,
            start_time,
            end_time,
            duration_ms,
            space: space.to_owned(),
            name: name.to_owned(),
            version: version.to_owned(),
            scope: scope_name.to_string(),
            span_name: span.name.clone(),
            span_kind: Self::span_kind_to_string(span.kind),
            status_code: span
                .status
                .as_ref()
                .map(|s| s.code.to_string())
                .unwrap_or_else(|| "Unset".to_string()),
            status_message: span
                .status
                .as_ref()
                .map(|s| s.message.clone())
                .unwrap_or_default(),
            attributes: serde_json::to_value(&span.attributes).unwrap_or(Value::Null),
            events: serde_json::to_value(&span.events).unwrap_or(Value::Null),
            links: serde_json::to_value(&span.links).unwrap_or(Value::Null),
        }
    }

    pub fn to_records(&self) -> TraceRecords {
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

        let space = &self.space;
        let name = &self.name;
        let version = &self.version;

        for resource_span in resource_spans {
            for scope_span in &resource_span.scope_spans {
                // Pre-compute scope name and attributes to avoid repeated work
                let (scope_name, scope_attributes) = match &scope_span.scope {
                    Some(scope) => (
                        scope.name.as_str(),
                        serde_json::to_value(&scope.attributes).ok(),
                    ),
                    None => ("", None),
                };

                for span in &scope_span.spans {
                    // no need to recalculate for every record type
                    let (start_time, end_time, duration_ms) =
                        Self::extract_time(span.start_time_unix_nano, span.end_time_unix_nano);

                    // TraceRecord for upsert
                    trace_records.push(self.convert_to_trace_record(
                        span,
                        scope_name,
                        scope_attributes.clone(),
                        space,
                        name,
                        version,
                        start_time,
                        end_time,
                        duration_ms,
                    ));

                    // SpanRecord for insert
                    span_records.push(self.convert_to_span_record(
                        span,
                        scope_name,
                        space,
                        name,
                        version,
                        start_time,
                        end_time,
                        duration_ms,
                    ));

                    // BaggageRecords for insert
                    baggage_records.extend(Self::convert_to_baggage_records(
                        span, scope_name, space, name, version,
                    ));
                }
            }
        }

        (trace_records, span_records, baggage_records)
    }
}

pub enum MessageType {
    Server,
    Trace,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageRecord {
    ServerRecords(ServerRecords),
    TraceServerRecord(TraceServerRecord),
}

impl MessageRecord {
    pub fn record_type(&self) -> MessageType {
        match self {
            MessageRecord::ServerRecords(_) => MessageType::Server,
            MessageRecord::TraceServerRecord(_) => MessageType::Trace,
        }
    }

    pub fn space(&self) -> String {
        match self {
            MessageRecord::ServerRecords(records) => records.space(),
            MessageRecord::TraceServerRecord(records) => records.space.clone(),
        }
    }
}
