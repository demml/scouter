use crate::core::cron::EveryDay;
use crate::core::dispatch::types::AlertDispatchType;
use crate::core::drift::spc::types::{SpcDriftProfile, SpcServerRecord};
use crate::core::error::{MonitorError, ScouterError};
use crate::core::observe::observer::ObservabilityMetrics;
use crate::core::utils::ProfileFuncs;
use ndarray::{Array, Array2};
use pyo3::prelude::*;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
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
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
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

pub trait CategoricalFeatureHelpers {
    // creates a feature map from a 2D array
    //
    // # Arguments
    //
    // * `features` - A vector of feature names
    // * `array` - A 2D array of string values
    //
    // # Returns
    //
    // A feature map
    fn create_feature_map(
        &self,
        features: &[String],
        array: &[Vec<String>],
    ) -> Result<FeatureMap, MonitorError> {
        // check if features and array are the same length
        if features.len() != array.len() {
            return Err(MonitorError::ShapeMismatchError(
                "Features and array are not the same length".to_string(),
            ));
        };

        let feature_map = array
            .par_iter()
            .enumerate()
            .map(|(i, col)| {
                let unique = col
                    .iter()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                let mut map = HashMap::new();
                for (j, item) in unique.iter().enumerate() {
                    map.insert(item.to_string(), j);

                    // check if j is last index
                    if j == unique.len() - 1 {
                        // insert missing value
                        map.insert("missing".to_string(), j + 1);
                    }
                }

                (features[i].to_string(), map)
            })
            .collect::<HashMap<_, _>>();

        Ok(FeatureMap {
            features: feature_map,
        })
    }

    fn convert_strings_to_ndarray_f32(
        &self,
        features: &Vec<String>,
        array: &[Vec<String>],
        feature_map: &FeatureMap,
    ) -> Result<Array2<f32>, MonitorError>
where {
        // check if features in feature_map.features.keys(). If any feature is not found, return error
        let features_not_exist = features
            .iter()
            .map(|x| feature_map.features.contains_key(x))
            .position(|x| !x);

        if features_not_exist.is_some() {
            return Err(MonitorError::MissingFeatureError(
                "Features provided do not exist in feature map".to_string(),
            ));
        }

        let data = features
            .par_iter()
            .enumerate()
            .map(|(i, feature)| {
                let map = feature_map.features.get(feature).unwrap();

                // attempt to set feature. If not found, set to missing
                let col = array[i]
                    .iter()
                    .map(|x| *map.get(x).unwrap_or(map.get("missing").unwrap()) as f32)
                    .collect::<Vec<_>>();

                col
            })
            .collect::<Vec<_>>();

        let data = Array::from_shape_vec((features.len(), array[0].len()), data.concat())
            .map_err(|e| MonitorError::ArrayError(e.to_string()))?;

        Ok(data.t().to_owned())
    }
    fn convert_strings_to_ndarray_f64(
        &self,
        features: &Vec<String>,
        array: &[Vec<String>],
        feature_map: &FeatureMap,
    ) -> Result<Array2<f64>, MonitorError>
where {
        // check if features in feature_map.features.keys(). If any feature is not found, return error
        let features_not_exist = features
            .iter()
            .map(|x| feature_map.features.contains_key(x))
            .position(|x| !x);

        if features_not_exist.is_some() {
            return Err(MonitorError::MissingFeatureError(
                "Features provided do not exist in feature map".to_string(),
            ));
        }
        let data = features
            .par_iter()
            .enumerate()
            .map(|(i, feature)| {
                let map = feature_map.features.get(feature).unwrap();

                // attempt to set feature. If not found, set to missing
                let col = array[i]
                    .iter()
                    .map(|x| *map.get(x).unwrap_or(map.get("missing").unwrap()) as f64)
                    .collect::<Vec<_>>();
                col
            })
            .collect::<Vec<_>>();

        let data = Array::from_shape_vec((features.len(), array[0].len()), data.concat())
            .map_err(|e| MonitorError::ArrayError(e.to_string()))?;

        Ok(data.t().to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
