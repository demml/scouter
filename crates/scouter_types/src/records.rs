use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use crate::ProfileFuncs;
use scouter_error::{ScouterError, PyScouterError};
use std::collections::HashMap;
use pyo3::IntoPyObjectExt;
use tracing::error;

#[pyclass(eq)]
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub enum RecordType {
    #[default]
    Spc,
    Psi,
    Observability,
    Custom,
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpcServerRecord {
    #[pyo3(get)]
    pub created_at: chrono::NaiveDateTime,

    #[pyo3(get)]
    pub repository: String,

    #[pyo3(get)]
    pub name: String,

    #[pyo3(get)]
    pub version: String,

    #[pyo3(get)]
    pub feature: String,

    #[pyo3(get)]
    pub value: f64,

    #[pyo3(get)]
    pub record_type: RecordType,
}

#[pymethods]
impl SpcServerRecord {
    #[new]
    pub fn new(
        repository: String,
        name: String,
        version: String,
        feature: String,
        value: f64,
    ) -> Self {
        Self {
            created_at: chrono::Utc::now().naive_utc(),
            name,
            repository,
            version,
            feature,
            value,
            record_type: RecordType::Spc,
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
        record.insert("repository".to_string(), self.repository.clone());
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
    pub created_at: chrono::NaiveDateTime,

    #[pyo3(get)]
    pub repository: String,

    #[pyo3(get)]
    pub name: String,

    #[pyo3(get)]
    pub version: String,

    #[pyo3(get)]
    pub feature: String,

    #[pyo3(get)]
    pub bin_id: String,

    #[pyo3(get)]
    pub bin_count: usize,

    #[pyo3(get)]
    pub record_type: RecordType,
}

#[pymethods]
impl PsiServerRecord {
    #[new]
    pub fn new(
        repository: String,
        name: String,
        version: String,
        feature: String,
        bin_id: String,
        bin_count: usize,
    ) -> Self {
        Self {
            created_at: chrono::Utc::now().naive_utc(),
            name,
            repository,
            version,
            feature,
            bin_id,
            bin_count,
            record_type: RecordType::Psi,
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
        record.insert("repository".to_string(), self.repository.clone());
        record.insert("version".to_string(), self.version.clone());
        record.insert("feature".to_string(), self.feature.clone());
        record.insert("bind_id".to_string(), self.bin_id.clone());
        record
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomMetricServerRecord {
    #[pyo3(get)]
    pub created_at: chrono::NaiveDateTime,

    #[pyo3(get)]
    pub repository: String,

    #[pyo3(get)]
    pub name: String,

    #[pyo3(get)]
    pub version: String,

    #[pyo3(get)]
    pub metric: String,

    #[pyo3(get)]
    pub value: f64,

    #[pyo3(get)]
    pub record_type: RecordType,
}

#[pymethods]
impl CustomMetricServerRecord {
    #[new]
    pub fn new(
        repository: String,
        name: String,
        version: String,
        metric: String,
        value: f64,
    ) -> Self {
        Self {
            created_at: chrono::Utc::now().naive_utc(),
            name,
            repository,
            version,
            metric: metric.to_lowercase(),
            value,
            record_type: RecordType::Custom,
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
        record.insert("repository".to_string(), self.repository.clone());
        record.insert("version".to_string(), self.version.clone());
        record.insert("metric".to_string(), self.metric.clone());
        record.insert("value".to_string(), self.value.to_string());
        record
    }
}

#[pyclass]
#[derive(Clone, Debug, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Serialize, Deserialize)]
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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObservabilityMetrics {
    #[pyo3(get)]
    pub repository: String,

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

    #[pyo3(get)]
    pub record_type: RecordType,
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
    Spc { record: SpcServerRecord },
    Psi { record: PsiServerRecord },
    Custom { record: CustomMetricServerRecord },
    Observability { record: ObservabilityMetrics },
}

#[pymethods]
impl ServerRecord {
    #[new]
    pub fn new(record: &Bound<'_, PyAny>) -> Self {
        let record_type: RecordType = record.getattr("record_type").unwrap().extract().unwrap();

        match record_type {
            RecordType::Spc => {
                let record: SpcServerRecord = record.extract().unwrap();
                ServerRecord::Spc { record }
            }
            RecordType::Psi => {
                let record: PsiServerRecord = record.extract().unwrap();
                ServerRecord::Psi { record }
            }
            RecordType::Custom => {
                let record: CustomMetricServerRecord = record.extract().unwrap();
                ServerRecord::Custom { record }
            }
            RecordType::Observability => {
                let record: ObservabilityMetrics = record.extract().unwrap();
                ServerRecord::Observability { record }
            }
        }
    }

    pub fn record(&self, py: Python) -> PyResult<PyObject> {
        match self {
            ServerRecord::Spc { record } => Ok(record
                .clone()
                .into_py_any(py)
                .map_err(PyScouterError::new_err)?),
            ServerRecord::Psi { record } => Ok(record
                .clone()
                .into_py_any(py)
                .map_err(PyScouterError::new_err)?),
            ServerRecord::Custom { record } => Ok(record
                .clone()
                .into_py_any(py)
                .map_err(PyScouterError::new_err)?),
            ServerRecord::Observability { record } => Ok(record
                .clone()
                .into_py_any(py)
                .map_err(PyScouterError::new_err)?),
        }
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerRecords {
    #[pyo3(get)]
    pub record_type: RecordType,

    #[pyo3(get)]
    pub records: Vec<ServerRecord>,
}

#[pymethods]
impl ServerRecords {
    #[new]
    pub fn new(records: Vec<ServerRecord>, record_type: RecordType) -> Self {
        Self {
            record_type,
            records,
        }
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
}


pub trait ToDriftRecords {
    fn to_spc_drift_records(&self) -> Result<Vec<SpcServerRecord>, ScouterError>;
    fn to_observability_drift_records(&self) -> Result<Vec<ObservabilityMetrics>, ScouterError>;
    fn to_psi_drift_records(&self) -> Result<Vec<PsiServerRecord>, ScouterError>;
    fn to_custom_metric_drift_records(&self) -> Result<Vec<CustomMetricServerRecord>, ScouterError>;
}
impl ToDriftRecords for ServerRecords {
    fn to_spc_drift_records(&self) -> Result<Vec<SpcServerRecord>, ScouterError> {
        match self.record_type {
            RecordType::Spc => {
                let mut records = Vec::new();
                for record in self.records.iter() {
                    match record {
                        ServerRecord::Spc {
                            record: inner_record,
                        } => {
                            records.push(inner_record.clone());
                        }
                        _ => {
                            error!("Unexpected record type");
                        }
                    }
                }
                Ok(records)
            }
            RecordType::Observability => Err(ScouterError::InvalidDriftTypeError("Unexpected record type".to_string())),
            RecordType::Psi => Err(ScouterError::InvalidDriftTypeError("Unexpected record type".to_string())),
            RecordType::Custom => Err(ScouterError::InvalidDriftTypeError("Unexpected record type".to_string())),
        }
    }

    fn to_observability_drift_records(&self) -> Result<Vec<ObservabilityMetrics>, ScouterError> {
        match self.record_type {
            RecordType::Spc => Err(ScouterError::InvalidDriftTypeError("Unexpected record type".to_string())),
            RecordType::Observability => {
                let mut records = Vec::new();
                for record in self.records.iter() {
                    match record {
                        ServerRecord::Observability {
                            record: inner_record,
                        } => {
                            records.push(inner_record.clone());
                        }
                        _ => {
                            error!("Unexpected record type");
                        }
                    }
                }
                Ok(records)
            }
            RecordType::Psi => Err(ScouterError::InvalidDriftTypeError("Unexpected record type".to_string())),
            RecordType::Custom => Err(ScouterError::InvalidDriftTypeError("Unexpected record type".to_string())),
        }
    }

    fn to_psi_drift_records(&self) -> Result<Vec<PsiServerRecord>, ScouterError> {
        match self.record_type {
            RecordType::Psi => {
                let mut records = Vec::new();
                for record in self.records.iter() {
                    match record {
                        ServerRecord::Psi {
                            record: inner_record,
                        } => {
                            records.push(inner_record.clone());
                        }
                        _ => {
                            error!("Unexpected record type");
                        }
                    }
                }
                Ok(records)
            }
            RecordType::Observability => Err(ScouterError::InvalidDriftTypeError("Unexpected record type".to_string())),
            RecordType::Spc => Err(ScouterError::InvalidDriftTypeError("Unexpected record type".to_string())),
            RecordType::Custom => Err(ScouterError::InvalidDriftTypeError("Unexpected record type".to_string())),
        }
    }

    fn to_custom_metric_drift_records(&self) -> Result<Vec<CustomMetricServerRecord>, ScouterError> {
        match self.record_type {
            RecordType::Custom => {
                let mut records = Vec::new();
                for record in self.records.iter() {
                    match record {
                        ServerRecord::Custom {
                            record: inner_record,
                        } => {
                            records.push(inner_record.clone());
                        }
                        _ => {
                            error!("Unexpected record type");
                        }
                    }
                }
                Ok(records)
            }
            RecordType::Observability => Err(ScouterError::InvalidDriftTypeError("Unexpected record type".to_string())),
            RecordType::Spc => Err(ScouterError::InvalidDriftTypeError("Unexpected record type".to_string())),
            RecordType::Psi => Err(ScouterError::InvalidDriftTypeError("Unexpected record type".to_string())),
        }
    }
}
