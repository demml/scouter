use crate::core::dispatch::types::AlertDispatchType;
use crate::core::utils::ProfileFuncs;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Trait for alert descriptions
// This is to be used for all kinds of feature alerts
pub trait DispatchAlertDescription {
    fn create_alert_description(&self, dispatch_type: AlertDispatchType) -> String;
}

pub struct DriftArgs {
    pub name: String,
    pub repository: String,
    pub version: String,
    pub dispatch_type: AlertDispatchType,
}

pub trait DispatchDriftConfig {
    fn get_drift_args(&self) -> DriftArgs;
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

        ProfileFuncs::__json__(self.records.clone())
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}
