use crate::core::dispatch::types::AlertDispatchType;
use crate::core::drift::spc::types::SpcServerRecord;
use crate::core::error::ScouterError;
use crate::core::utils::ProfileFuncs;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, str::FromStr};

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum DriftType {
    SPC,
    PSI,
}

#[pymethods]
impl DriftType {
    #[getter]
    pub fn value(&self) -> String {
        match self {
            DriftType::SPC => "SPC".to_string(),
            DriftType::PSI => "PSI".to_string(),
        }
    }
}

impl FromStr for DriftType {
    type Err = ScouterError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "SPC" => Ok(DriftType::SPC),
            "PSI" => Ok(DriftType::PSI),
            _ => Err(ScouterError::InvalidDriftTypeError(s.to_string())),
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

// Trait for drift records
pub trait DriftRecordType {
    fn get_drift_type(&self) -> DriftType;
}

pub struct ProfileArgs {
    pub name: String,
    pub repository: String,
    pub version: String,
    pub schedule: String,
    pub scouter_version: String,
    pub profile_type: DriftType,
}

// trait to implement on all profile types
pub trait ProfileBaseArgs {
    fn get_base_args(&self) -> ProfileArgs;
    fn to_value(&self) -> serde_json::Value;
}

pub struct DriftArgs {
    pub name: String,
    pub repository: String,
    pub version: String,
    pub dispatch_type: AlertDispatchType,
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

#[pymethods]
impl ServerRecord {
    #[new]
    pub fn new(record: SpcServerRecord) -> Self {
        ServerRecord::DRIFT { record }
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

impl DriftRecordType for ServerRecords {
    // Gets the drift type of the records. Primarily used for inserting records into scouter-server db
    fn get_drift_type(&self) -> DriftType {
        match self.record_type {
            RecordType::DRIFT => match self.records.first().unwrap() {
                ServerRecord::DRIFT { record: _ } => DriftType::SPC,
            },
            _ => DriftType::SPC,
        }
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeatureMap {
    #[pyo3(get)]
    pub features: HashMap<String, HashMap<String, usize>>,
}

#[pymethods]
impl FeatureMap {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}
