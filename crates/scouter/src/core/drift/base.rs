use crate::core::dispatch::types::AlertDispatchType;
use crate::core::error::ScouterError;
use crate::core::utils::ProfileFuncs;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum DriftType {
    SPC,
    PSI,
    NONE,
}

#[pymethods]
impl DriftType {
    #[getter]
    pub fn value(&self) -> String {
        match self {
            DriftType::SPC => "SPC".to_string(),
            DriftType::PSI => "PSI".to_string(),
            DriftType::NONE => "NONE".to_string(),
        }
    }
}

// Trait for alert descriptions
// This is to be used for all kinds of feature alerts
pub trait DispatchAlertDescription {
    fn create_alert_description(&self, dispatch_type: AlertDispatchType) -> String;
}

pub trait DispatchDriftConfig {
    fn get_drift_args(&self) -> DriftArgs;
}

pub trait DriftRecordType {
    fn get_drift_type(&self) -> DriftType;
}

pub struct DriftArgs {
    pub name: String,
    pub repository: String,
    pub version: String,
    pub dispatch_type: AlertDispatchType,
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpcServerRecord {
    #[pyo3(get)]
    pub created_at: chrono::NaiveDateTime,

    #[pyo3(get)]
    pub name: String,

    #[pyo3(get)]
    pub repository: String,

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
    pub fn new(
        name: String,
        repository: String,
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
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub enum RecordType {
    #[default]
    DRIFT,
    OBSERVABILITY,
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ServerRecord {
    DRIFT { record: SpcServerRecord },
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

impl DriftRecordType for ServerRecords {
    // Gets the drift type of the records. Primarily used for inserting records into scouter-server db
    fn get_drift_type(&self) -> DriftType {
        match self.record_type {
            RecordType::DRIFT => match self.records.first().unwrap() {
                ServerRecord::DRIFT { record: _ } => DriftType::SPC,
            },
            _ => DriftType::NONE,
        }
    }
}
