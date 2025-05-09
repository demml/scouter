use crate::ProfileFuncs;
use chrono::DateTime;
use chrono::Utc;
use pyo3::prelude::*;
use pyo3::IntoPyObjectExt;
use scouter_error::DriftError;
use scouter_error::{PyScouterError, ScouterError};
use serde::{Deserialize, Serialize};
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
}

impl Display for RecordType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecordType::Spc => write!(f, "spc"),
            RecordType::Psi => write!(f, "psi"),
            RecordType::Observability => write!(f, "observability"),
            RecordType::Custom => write!(f, "custom"),
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

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__json__(self)
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
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ServerRecord {
    Spc(SpcServerRecord),
    Psi(PsiServerRecord),
    Custom(CustomMetricServerRecord),
    Observability(ObservabilityMetrics),
}

#[pymethods]
impl ServerRecord {
    #[new]
    pub fn new(record: &Bound<'_, PyAny>) -> PyResult<Self> {
        if let Ok(spc_record) = record.extract::<SpcServerRecord>() {
            return Ok(ServerRecord::Spc(spc_record));
        }

        if let Ok(psi_record) = record.extract::<PsiServerRecord>() {
            return Ok(ServerRecord::Psi(psi_record));
        }

        if let Ok(custom_record) = record.extract::<CustomMetricServerRecord>() {
            return Ok(ServerRecord::Custom(custom_record));
        }

        if let Ok(observability_record) = record.extract::<ObservabilityMetrics>() {
            return Ok(ServerRecord::Observability(observability_record));
        }

        // If none of the extractions succeeded, return an error
        Err(PyScouterError::new_err(
            "Unable to extract record into any known ServerRecord variant",
        ))
    }

    #[getter]
    pub fn record(&self, py: Python) -> PyResult<PyObject> {
        match self {
            ServerRecord::Spc(record) => Ok(record
                .clone()
                .into_py_any(py)
                .map_err(PyScouterError::new_err)?),
            ServerRecord::Psi(record) => Ok(record
                .clone()
                .into_py_any(py)
                .map_err(PyScouterError::new_err)?),
            ServerRecord::Custom(record) => Ok(record
                .clone()
                .into_py_any(py)
                .map_err(PyScouterError::new_err)?),
            ServerRecord::Observability(record) => Ok(record
                .clone()
                .into_py_any(py)
                .map_err(PyScouterError::new_err)?),
        }
    }

    pub fn space(&self) -> String {
        match self {
            ServerRecord::Spc(record) => record.space.clone(),
            ServerRecord::Psi(record) => record.space.clone(),
            ServerRecord::Custom(record) => record.space.clone(),
            ServerRecord::Observability(record) => record.space.clone(),
        }
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        match self {
            ServerRecord::Spc(record) => record.__str__(),
            ServerRecord::Psi(record) => record.__str__(),
            ServerRecord::Custom(record) => record.__str__(),
            ServerRecord::Observability(record) => record.__str__(),
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
    pub fn record_type(&self) -> Result<RecordType, ScouterError> {
        if let Some(first) = self.records.first() {
            match first {
                ServerRecord::Spc(_) => Ok(RecordType::Spc),
                ServerRecord::Psi(_) => Ok(RecordType::Psi),
                ServerRecord::Custom(_) => Ok(RecordType::Custom),
                ServerRecord::Observability(_) => Ok(RecordType::Observability),
            }
        } else {
            Err(ScouterError::EmptyServerRecordsError)
        }
    }

    // Helper function to load records from bytes. Used by scouter-server consumers
    //
    // # Arguments
    //
    // * `bytes` - A slice of bytes
    pub fn load_from_bytes(bytes: &[u8]) -> Result<Self, ScouterError> {
        let records: ServerRecords =
            serde_json::from_slice(bytes).map_err(|_| ScouterError::DeSerializeError)?;
        Ok(records)
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn unique_dates(&self) -> Result<Vec<DateTime<Utc>>, ScouterError> {
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
                    return Err(ScouterError::InvalidDriftTypeError(
                        "Unexpected record type".to_string(),
                    ));
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
    fn to_spc_drift_records(&self) -> Result<Vec<SpcServerRecord>, DriftError>;
    fn to_observability_drift_records(&self) -> Result<Vec<ObservabilityMetrics>, DriftError>;
    fn to_psi_drift_records(&self) -> Result<Vec<PsiServerRecord>, DriftError>;
    fn to_custom_metric_drift_records(&self) -> Result<Vec<CustomMetricServerRecord>, DriftError>;
}
impl ToDriftRecords for ServerRecords {
    fn to_spc_drift_records(&self) -> Result<Vec<SpcServerRecord>, DriftError> {
        extract_records(self, |record| match record {
            ServerRecord::Spc(inner) => Some(inner.clone()),
            _ => None,
        })
    }

    fn to_observability_drift_records(&self) -> Result<Vec<ObservabilityMetrics>, DriftError> {
        extract_records(self, |record| match record {
            ServerRecord::Observability(inner) => Some(inner.clone()),
            _ => None,
        })
    }

    fn to_psi_drift_records(&self) -> Result<Vec<PsiServerRecord>, DriftError> {
        extract_records(self, |record| match record {
            ServerRecord::Psi(inner) => Some(inner.clone()),
            _ => None,
        })
    }

    fn to_custom_metric_drift_records(&self) -> Result<Vec<CustomMetricServerRecord>, DriftError> {
        extract_records(self, |record| match record {
            ServerRecord::Custom(inner) => Some(inner.clone()),
            _ => None,
        })
    }
}

// Helper function to extract records of a specific type
fn extract_records<T>(
    server_records: &ServerRecords,
    extractor: impl Fn(&ServerRecord) -> Option<T>,
) -> Result<Vec<T>, DriftError> {
    let mut records = Vec::new();

    for record in &server_records.records {
        if let Some(extracted) = extractor(record) {
            records.push(extracted);
        } else {
            return Err(DriftError::InvalidDriftTypeError);
        }
    }

    Ok(records)
}
