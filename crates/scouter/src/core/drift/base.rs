use crate::core::dispatch::types::AlertDispatchType;
use crate::core::drift::spc::types::{SpcDriftProfile, SpcServerRecord};
use crate::core::error::ScouterError;
use crate::core::observe::observer::ObservabilityMetrics;
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

#[derive(PartialEq, Debug)]
pub struct ProfileArgs {
    pub name: String,
    pub repository: String,
    pub version: String,
    pub schedule: String,
    pub scouter_version: String,
    pub drift_type: DriftType,
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
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub enum RecordType {
    #[default]
    SPC,
    PSI,
    OBSERVABILITY,
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ServerRecord {
    SPC { record: SpcServerRecord },
    OBSERVABILITY { record: ObservabilityMetrics },
}

#[pymethods]
impl ServerRecord {
    #[new]
    pub fn new(record: SpcServerRecord) -> Self {
        ServerRecord::SPC { record }
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

// Generic enum to be used on scouter server
#[derive(Debug, Clone)]
pub enum DriftProfile {
    SpcDriftProfile(SpcDriftProfile),
}

impl DriftProfile {
    /// Create a new DriftProfile from a DriftType and a profile string
    /// This function will map the drift type to the correct profile type to load
    ///
    /// # Arguments
    ///
    /// * `drift_type` - DriftType enum
    /// * `profile` - Profile string
    ///
    /// # Returns
    ///
    /// * `Result<Self>` - Result of DriftProfile
    pub fn from_str(drift_type: DriftType, profile: String) -> Result<Self, ScouterError> {
        match drift_type {
            DriftType::SPC => {
                let profile =
                    serde_json::from_str(&profile).map_err(|_| ScouterError::DeSerializeError)?;
                Ok(DriftProfile::SpcDriftProfile(profile))
            }
            DriftType::PSI => todo!(),
        }
    }

    /// Get the base arguments for a drift profile
    pub fn get_base_args(&self) -> ProfileArgs {
        match self {
            DriftProfile::SpcDriftProfile(profile) => profile.get_base_args(),
        }
    }

    pub fn to_value(&self) -> serde_json::Value {
        match self {
            DriftProfile::SpcDriftProfile(profile) => profile.to_value(),
        }
    }

    /// Create a new DriftProfile from a value (this is used by scouter-server)
    /// This function will map the drift type to the correct profile type to load
    ///
    /// # Arguments
    ///
    /// * `body` - Request body
    /// * `drift_type` - Drift type string
    ///
    pub fn from_value(body: serde_json::Value, drift_type: &str) -> Result<Self, ScouterError> {
        let drift_type = DriftType::from_str(drift_type)?;
        match drift_type {
            DriftType::SPC => {
                let profile =
                    serde_json::from_value(body).map_err(|_| ScouterError::DeSerializeError)?;
                Ok(DriftProfile::SpcDriftProfile(profile))
            }
            DriftType::PSI => todo!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_drift_type_from_str() {
        assert_eq!(DriftType::from_str("SPC").unwrap(), DriftType::SPC);
        assert_eq!(DriftType::from_str("PSI").unwrap(), DriftType::PSI);
        assert!(DriftType::from_str("INVALID").is_err());
    }

    #[test]
    fn test_drift_type_value() {
        assert_eq!(DriftType::SPC.value(), "SPC");
        assert_eq!(DriftType::PSI.value(), "PSI");
    }
}
