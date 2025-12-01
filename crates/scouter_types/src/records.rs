use crate::error::RecordError;
use crate::trace::TraceServerRecord;
use crate::DriftType;
use crate::PyHelperFuncs;
use crate::Status;
use crate::TagRecord;
use chrono::DateTime;
use chrono::Utc;
use potato_head::create_uuid7;
use pyo3::prelude::*;
use pyo3::IntoPyObjectExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Display;

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

impl RecordType {
    pub fn to_drift_type(&self) -> &str {
        match self {
            RecordType::Spc => DriftType::Spc.to_string(),
            RecordType::Psi => DriftType::Psi.to_string(),
            RecordType::Custom => DriftType::Custom.to_string(),
            RecordType::LLMDrift => DriftType::LLM.to_string(),
            RecordType::LLMMetric => DriftType::LLM.to_string(),
            _ => "unknown",
        }
    }
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
pub struct SpcRecord {
    #[pyo3(get)]
    pub created_at: chrono::DateTime<Utc>,

    #[pyo3(get)]
    pub uid: String,

    #[pyo3(get)]
    pub feature: String,

    #[pyo3(get)]
    pub value: f64,
}

#[pymethods]
impl SpcRecord {
    #[new]
    pub fn new(uid: String, feature: String, value: f64) -> Self {
        Self {
            created_at: Utc::now(),
            uid,
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
        record.insert("uid".to_string(), self.uid.clone());
        record.insert("feature".to_string(), self.feature.clone());
        record.insert("value".to_string(), self.value.to_string());
        record
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpcInternalRecord {
    pub created_at: chrono::DateTime<Utc>,
    pub entity_id: i32,
    pub feature: String,
    pub value: f64,
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PsiRecord {
    #[pyo3(get)]
    pub created_at: chrono::DateTime<Utc>,
    #[pyo3(get)]
    pub uid: String,
    #[pyo3(get)]
    pub feature: String,
    #[pyo3(get)]
    pub bin_id: usize,
    #[pyo3(get)]
    pub bin_count: usize,
}

#[pymethods]
impl PsiRecord {
    #[new]
    pub fn new(uid: String, feature: String, bin_id: usize, bin_count: usize) -> Self {
        Self {
            created_at: Utc::now(),
            uid,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PsiInternalRecord {
    pub created_at: chrono::DateTime<Utc>,
    pub entity_id: i32,
    pub feature: String,
    pub bin_id: usize,
    pub bin_count: usize,
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LLMDriftRecord {
    #[pyo3(get)]
    pub created_at: chrono::DateTime<Utc>,

    #[pyo3(get)]
    pub uid: String,

    pub prompt: Option<Value>,

    pub context: Value,

    pub status: Status,

    pub id: i64,

    pub score: Value,

    pub updated_at: Option<DateTime<Utc>>,

    pub processing_started_at: Option<DateTime<Utc>>,

    pub processing_ended_at: Option<DateTime<Utc>>,

    pub processing_duration: Option<i32>,

    // this is used to lookup entity_id from entity table
    pub entity_uid: String,
}

#[pymethods]
impl LLMDriftRecord {
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

impl LLMDriftRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new_rs(
        prompt: Option<Value>,
        context: Value,
        created_at: DateTime<Utc>,
        score: Value,
        uid: String,
        entity_uid: String,
    ) -> Self {
        Self {
            created_at,
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
            entity_uid,
        }
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BoxedLLMDriftRecord {
    pub record: Box<LLMDriftRecord>,
}

impl BoxedLLMDriftRecord {
    pub fn new(record: LLMDriftRecord) -> Self {
        Self {
            record: Box::new(record),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct LLMDriftInternalRecord {
    pub created_at: chrono::DateTime<Utc>,
    pub entity_id: i32,     // foreign key to entity table
    pub entity_uid: String, // public unique identifier for entity
    pub uid: String,        // public unique identifier for drift record
    pub prompt: Option<Value>,
    pub context: Value,
    #[cfg_attr(feature = "server", sqlx(try_from = "String"))]
    pub status: Status,
    pub id: i64,
    pub score: Value,
    pub updated_at: Option<DateTime<Utc>>,
    pub processing_started_at: Option<DateTime<Utc>>,
    pub processing_ended_at: Option<DateTime<Utc>>,
    pub processing_duration: Option<i32>,
}

impl LLMDriftInternalRecord {
    pub fn from_server_record(record: &LLMDriftRecord, entity_id: i32) -> Self {
        Self {
            created_at: record.created_at,
            prompt: record.prompt.clone(),
            context: record.context.clone(),
            score: record.score.clone(),
            status: record.status.clone(),
            id: 0,
            uid: create_uuid7(),
            updated_at: None,
            processing_started_at: None,
            processing_ended_at: None,
            processing_duration: None,
            entity_id,
            entity_uid: record.entity_uid.clone(),
        }
    }

    pub fn to_public_record(&self) -> LLMDriftRecord {
        LLMDriftRecord {
            created_at: self.created_at,
            uid: self.uid.clone(),
            prompt: self.prompt.clone(),
            context: self.context.clone(),
            status: self.status.clone(),
            id: self.id,
            score: self.score.clone(),
            updated_at: self.updated_at,
            processing_started_at: self.processing_started_at,
            processing_ended_at: self.processing_ended_at,
            processing_duration: self.processing_duration,
            entity_uid: self.entity_uid.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BoxedLLMDriftInternalRecord {
    pub record: Box<LLMDriftInternalRecord>,
}

impl BoxedLLMDriftInternalRecord {
    pub fn new(record: LLMDriftInternalRecord) -> Self {
        Self {
            record: Box::new(record),
        }
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LLMMetricRecord {
    #[pyo3(get)]
    pub uid: String,

    #[pyo3(get)]
    pub created_at: chrono::DateTime<Utc>,

    #[pyo3(get)]
    pub entity_uid: String,

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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LLMMetricInternalRecord {
    pub entity_uid: String,
    pub uid: String,
    pub created_at: chrono::DateTime<Utc>,
    pub entity_id: i32,
    pub metric: String,
    pub value: f64,
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomMetricRecord {
    #[pyo3(get)]
    pub created_at: chrono::DateTime<Utc>,
    #[pyo3(get)]
    pub uid: String,
    #[pyo3(get)]
    pub metric: String,
    #[pyo3(get)]
    pub value: f64,
}

#[pymethods]
impl CustomMetricRecord {
    #[new]
    pub fn new(uid: String, metric: String, value: f64) -> Self {
        Self {
            created_at: chrono::Utc::now(),
            uid,
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
        record.insert("uid".to_string(), self.uid.clone());
        record.insert("metric".to_string(), self.metric.clone());
        record.insert("value".to_string(), self.value.to_string());
        record
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomMetricInternalRecord {
    pub created_at: chrono::DateTime<Utc>,
    pub entity_id: i32,
    pub metric: String,
    pub value: f64,
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
    pub uid: String,

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

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ObservabilityMetricsInternal {
    pub entity_id: i32,
    pub request_count: i64,
    pub error_count: i64,
    pub route_metrics: Vec<RouteMetrics>,
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ServerRecord {
    Spc(SpcRecord),
    Psi(PsiRecord),
    Custom(CustomMetricRecord),
    Observability(ObservabilityMetrics),
    LLMDrift(BoxedLLMDriftRecord),
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
                let spc_record = record.extract::<SpcRecord>()?;
                Ok(ServerRecord::Spc(spc_record))
            }
            RecordType::Psi => {
                let psi_record = record.extract::<PsiRecord>()?;
                Ok(ServerRecord::Psi(psi_record))
            }
            RecordType::Custom => {
                let custom_record = record.extract::<CustomMetricRecord>()?;
                Ok(ServerRecord::Custom(custom_record))
            }
            RecordType::Observability => {
                let observability_record = record.extract::<ObservabilityMetrics>()?;
                Ok(ServerRecord::Observability(observability_record))
            }
            RecordType::LLMDrift => {
                let llm_drift_record = record.extract::<LLMDriftRecord>()?;
                Ok(ServerRecord::LLMDrift(BoxedLLMDriftRecord::new(
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum InternalServerRecord {
    Spc(SpcInternalRecord),
    Psi(PsiInternalRecord),
    Custom(CustomMetricInternalRecord),
    LLMDrift(BoxedLLMDriftInternalRecord),
    LLMMetric(LLMMetricInternalRecord),
    Observability(ObservabilityMetricsInternal),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InternalServerRecords {
    pub records: Vec<InternalServerRecord>,
}

impl InternalServerRecords {
    pub fn new(records: Vec<InternalServerRecord>) -> Self {
        Self { records }
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

    /// gets the uid from the first record type found in the records
    /// This is a helper for consumers that need to get an entity_id associated with the given uid
    pub fn uid(&self) -> Result<&String, RecordError> {
        if let Some(first) = self.records.first() {
            match first {
                ServerRecord::Spc(inner) => Ok(&inner.uid),
                ServerRecord::Psi(inner) => Ok(&inner.uid),
                ServerRecord::Custom(inner) => Ok(&inner.uid),
                ServerRecord::Observability(inner) => Ok(&inner.uid),
                ServerRecord::LLMDrift(inner) => Ok(&inner.record.entity_uid),
                ServerRecord::LLMMetric(inner) => Ok(&inner.uid),
            }
        } else {
            Err(RecordError::EmptyServerRecordsError)
        }
    }
}

/// Helper trait to convert ServerRecord to their respective internal record types
pub trait ToDriftRecords {
    fn to_spc_drift_records(&self) -> Result<Vec<SpcRecord>, RecordError>;
    fn to_observability_drift_records(&self) -> Result<Vec<ObservabilityMetrics>, RecordError>;
    fn to_psi_drift_records(&self) -> Result<Vec<PsiRecord>, RecordError>;
    fn to_custom_metric_drift_records(&self) -> Result<Vec<CustomMetricRecord>, RecordError>;
    fn to_llm_drift_records(&self) -> Result<Vec<LLMDriftRecord>, RecordError>;
    fn to_llm_metric_records(&self) -> Result<Vec<LLMMetricRecord>, RecordError>;
}

impl ToDriftRecords for ServerRecords {
    fn to_spc_drift_records(&self) -> Result<Vec<SpcRecord>, RecordError> {
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

    fn to_psi_drift_records(&self) -> Result<Vec<PsiRecord>, RecordError> {
        extract_records(self, |record| match record {
            ServerRecord::Psi(inner) => Some(inner.clone()),
            _ => None,
        })
    }

    fn to_custom_metric_drift_records(&self) -> Result<Vec<CustomMetricRecord>, RecordError> {
        extract_records(self, |record| match record {
            ServerRecord::Custom(inner) => Some(inner.clone()),
            _ => None,
        })
    }

    fn to_llm_drift_records(&self) -> Result<Vec<LLMDriftRecord>, RecordError> {
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

pub trait ToInternalDriftRecords {
    fn to_spc_drift_records(&self) -> Result<Vec<SpcInternalRecord>, RecordError>;
    fn to_observability_drift_records(
        &self,
    ) -> Result<Vec<ObservabilityMetricsInternal>, RecordError>;
    fn to_psi_drift_records(&self) -> Result<Vec<PsiInternalRecord>, RecordError>;
    fn to_custom_metric_drift_records(
        &self,
    ) -> Result<Vec<CustomMetricInternalRecord>, RecordError>;
    fn to_llm_drift_records(&self) -> Result<Vec<LLMDriftInternalRecord>, RecordError>;
    fn to_llm_metric_records(&self) -> Result<Vec<LLMMetricInternalRecord>, RecordError>;
}

impl ToInternalDriftRecords for InternalServerRecords {
    fn to_spc_drift_records(&self) -> Result<Vec<SpcInternalRecord>, RecordError> {
        extract_internal_records(self, |record| match record {
            InternalServerRecord::Spc(inner) => Some(inner.clone()),
            _ => None,
        })
    }

    fn to_observability_drift_records(
        &self,
    ) -> Result<Vec<ObservabilityMetricsInternal>, RecordError> {
        extract_internal_records(self, |record| match record {
            InternalServerRecord::Observability(inner) => Some(inner.clone()),
            _ => None,
        })
    }

    fn to_psi_drift_records(&self) -> Result<Vec<PsiInternalRecord>, RecordError> {
        extract_internal_records(self, |record| match record {
            InternalServerRecord::Psi(inner) => Some(inner.clone()),
            _ => None,
        })
    }

    fn to_custom_metric_drift_records(
        &self,
    ) -> Result<Vec<CustomMetricInternalRecord>, RecordError> {
        extract_internal_records(self, |record| match record {
            InternalServerRecord::Custom(inner) => Some(inner.clone()),
            _ => None,
        })
    }

    fn to_llm_drift_records(&self) -> Result<Vec<LLMDriftInternalRecord>, RecordError> {
        extract_internal_records(self, |record| match record {
            InternalServerRecord::LLMDrift(inner) => Some(*inner.record.clone()),
            _ => None,
        })
    }

    fn to_llm_metric_records(&self) -> Result<Vec<LLMMetricInternalRecord>, RecordError> {
        extract_internal_records(self, |record| match record {
            InternalServerRecord::LLMMetric(inner) => Some(inner.clone()),
            _ => None,
        })
    }
}

// Helper function to extract records of a specific type
fn extract_internal_records<T>(
    server_records: &InternalServerRecords,
    extractor: impl Fn(&InternalServerRecord) -> Option<T>,
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

pub enum MessageType {
    Server,
    Trace,
    Tag,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageRecord {
    ServerRecords(ServerRecords),
    TraceServerRecord(TraceServerRecord),
    TagServerRecord(TagRecord),
}

impl MessageRecord {
    pub fn record_type(&self) -> MessageType {
        match self {
            MessageRecord::ServerRecords(_) => MessageType::Server,
            MessageRecord::TraceServerRecord(_) => MessageType::Trace,
            MessageRecord::TagServerRecord(_) => MessageType::Tag,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            MessageRecord::ServerRecords(records) => records.len(),
            MessageRecord::TraceServerRecord(_) => 1,
            _ => 1,
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            MessageRecord::ServerRecords(records) => records.is_empty(),
            _ => false,
        }
    }

    pub fn model_dump_json(&self) -> String {
        // serialize records to a string
        match self {
            MessageRecord::ServerRecords(records) => records.model_dump_json(),
            MessageRecord::TraceServerRecord(record) => PyHelperFuncs::__json__(record),
            MessageRecord::TagServerRecord(record) => PyHelperFuncs::__json__(record),
        }
    }
}

/// implement iterator for MEssageRecord to iterate over ServerRecords.records
impl MessageRecord {
    pub fn iter_server_records(&self) -> Option<impl Iterator<Item = &ServerRecord>> {
        match self {
            MessageRecord::ServerRecords(records) => Some(records.records.iter()),
            _ => None,
        }
    }
}
