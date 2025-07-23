use crate::error::RecordError;
use crate::util::pyobject_to_json;
use crate::ProfileFuncs;
use crate::Status;
use chrono::DateTime;
use chrono::Utc;
use potato_head::create_uuid7;
use potato_head::Prompt;
use pyo3::prelude::*;
use pyo3::types::PyDict;
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
        ProfileFuncs::__str__(self)
    }

    pub fn get_record_type(&self) -> RecordType {
        RecordType::Spc
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__json__(self)
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
        ProfileFuncs::__str__(self)
    }

    pub fn get_record_type(&self) -> RecordType {
        RecordType::Psi
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__json__(self)
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

    pub input: Value,

    pub response: Value,

    pub context: Value,

    pub status: Status,

    pub id: i64,

    pub uid: String,

    pub updated_at: Option<DateTime<Utc>>,
    pub processing_started_at: Option<DateTime<Utc>>,
    pub processing_ended_at: Option<DateTime<Utc>>,
}

#[pymethods]
impl LLMDriftServerRecord {
    #[new]
    #[pyo3(signature = (
        space,
        name,
        version,
        input,
        response,
        prompt= None,
        context = None,
    ))]

    pub fn new(
        space: String,
        name: String,
        version: String,
        input: Bound<'_, PyAny>,
        response: Bound<'_, PyAny>,
        prompt: Option<Bound<'_, PyAny>>,
        context: Option<Bound<'_, PyDict>>,
    ) -> Result<Self, RecordError> {
        // Check if pydict was provided, if not, create an empty Map
        let context_val = context
            .map(|c| pyobject_to_json(&c))
            .unwrap_or(Ok(Value::Object(serde_json::Map::new())))?;

        let input = pyobject_to_json(&input)?;
        let response = pyobject_to_json(&response)?;

        // if prompt is provided, check if it is a valid Prompt object
        let prompt: Option<Value> = match prompt {
            Some(p) => {
                if p.is_instance_of::<Prompt>() {
                    let prompt = p.extract::<Prompt>()?;
                    Some(serde_json::to_value(prompt)?)
                } else {
                    Some(pyobject_to_json(&p)?)
                }
            }
            None => None,
        };

        Ok(Self {
            created_at: Utc::now(),
            space,
            name,
            version,
            input,
            response,
            prompt,
            context: context_val,
            status: Status::Pending,
            id: 0, // This is a placeholder, as the ID will be set by the database
            uid: create_uuid7(),
            updated_at: None,
            processing_started_at: None,
            processing_ended_at: None,
        })
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }

    pub fn get_record_type(&self) -> RecordType {
        RecordType::LLMDrift
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__json__(self)
    }
}

impl LLMDriftServerRecord {
    pub fn new_rs(
        space: String,
        name: String,
        version: String,
        input: Value,
        response: Value,
        prompt: Option<Value>,
        context: Value,
        created_at: DateTime<Utc>,
        uid: String,
    ) -> Self {
        Self {
            created_at,
            space,
            name,
            version,
            input,
            response,
            prompt,
            context,
            status: Status::Pending,
            id: 0, // This is a placeholder, as the ID will be set by the database
            uid,
            updated_at: None,
            processing_started_at: None,
            processing_ended_at: None,
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
pub struct LLMMetricServerRecord {
    pub created_at: chrono::DateTime<Utc>,
    pub space: String,
    pub name: String,
    pub version: String,
    pub metric: String,
    pub value: f64,
}

#[pymethods]
impl LLMMetricServerRecord {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
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
        ProfileFuncs::__str__(self)
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__json__(self)
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
        ProfileFuncs::__json__(self)
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
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
    LLMMetric(LLMMetricServerRecord),
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
    pub fn record(&self, py: Python) -> Result<PyObject, RecordError> {
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
        ProfileFuncs::__json__(self)
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
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
    fn to_llm_metric_records(&self) -> Result<Vec<LLMMetricServerRecord>, RecordError>;
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

    fn to_llm_metric_records(&self) -> Result<Vec<LLMMetricServerRecord>, RecordError> {
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
