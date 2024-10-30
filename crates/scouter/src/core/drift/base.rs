use crate::core::cron::EveryDay;
use crate::core::dispatch::types::AlertDispatchType;
use crate::core::drift::spc::types::{SpcDriftProfile, SpcServerRecord};
use crate::core::error::ScouterError;
use crate::core::observe::observer::ObservabilityMetrics;
use crate::core::utils::ProfileFuncs;

use pyo3::prelude::*;

use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum DriftType {
    SPC,
    PSI,
}

#[pymethods]
impl DriftType {
    #[staticmethod]
    pub fn from_value(value: &str) -> Option<Self> {
        match value {
            "SPC" => Some(DriftType::SPC),
            "PSI" => Some(DriftType::PSI),
            _ => None,
        }
    }

    #[getter]
    pub fn value(&self) -> &str {
        match self {
            DriftType::SPC => "SPC",
            DriftType::PSI => "PSI",
        }
    }
}

impl FromStr for DriftType {
    type Err = ScouterError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "SPC" => Ok(DriftType::SPC),
            "PSI" => Ok(DriftType::PSI),
            _ => Err(ScouterError::InvalidDriftTypeError(value.to_string())),
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

pub trait ValidateAlertConfig {
    fn resolve_schedule(schedule: Option<String>) -> String {
        let default_schedule = EveryDay::new().cron;

        match schedule {
            Some(s) => {
                cron::Schedule::from_str(&s) // Pass by reference here
                    .map(|_| s) // If valid, return the schedule
                    .unwrap_or_else(|_| {
                        tracing::error!("Invalid cron schedule, using default schedule");
                        default_schedule
                    })
            }
            None => default_schedule,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::utils::CategoricalFeatureHelpers;
    use std::str::FromStr;

    pub struct TestStruct;
    impl ValidateAlertConfig for TestStruct {}
    impl CategoricalFeatureHelpers for TestStruct {}

    #[test]
    fn test_resolve_schedule() {
        let valid_schedule = "0 0 5 * * *".to_string(); // Every day at 5:00 AM

        let result = TestStruct::resolve_schedule(Some(valid_schedule));

        assert_eq!(result, "0 0 5 * * *".to_string());

        let invalid_schedule = "invalid_cron".to_string();

        let default_schedule = EveryDay::new().cron;

        let result = TestStruct::resolve_schedule(Some(invalid_schedule));

        assert_eq!(result, default_schedule);
    }

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

    #[test]
    fn test_create_feature_map() {
        let string_vec = vec![
            vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
                "e".to_string(),
            ],
            vec![
                "hello".to_string(),
                "blah".to_string(),
                "c".to_string(),
                "d".to_string(),
                "e".to_string(),
                "hello".to_string(),
                "blah".to_string(),
                "c".to_string(),
                "d".to_string(),
                "e".to_string(),
            ],
        ];

        let string_features = vec!["feature_1".to_string(), "feature_2".to_string()];

        let feature_map = TestStruct
            .create_feature_map(&string_features, &string_vec)
            .unwrap();

        assert_eq!(feature_map.features.len(), 2);
        assert_eq!(feature_map.features.get("feature_2").unwrap().len(), 6);
    }

    #[test]
    fn test_create_array_from_string() {
        let string_vec = vec![
            vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
                "e".to_string(),
            ],
            vec![
                "a".to_string(),
                "a".to_string(),
                "a".to_string(),
                "b".to_string(),
                "b".to_string(),
            ],
        ];

        let string_features = vec!["feature_1".to_string(), "feature_2".to_string()];

        let feature_map = TestStruct
            .create_feature_map(&string_features, &string_vec)
            .unwrap();

        assert_eq!(feature_map.features.len(), 2);

        let f32_array = TestStruct
            .convert_strings_to_ndarray_f32(&string_features, &string_vec, &feature_map)
            .unwrap();

        assert_eq!(f32_array.shape(), &[5, 2]);

        let f64_array = TestStruct
            .convert_strings_to_ndarray_f64(&string_features, &string_vec, &feature_map)
            .unwrap();

        assert_eq!(f64_array.shape(), &[5, 2]);
    }
}
