#![allow(clippy::useless_conversion)]
use crate::error::{ProfileError, TypeError};
use crate::spc::alert::SpcAlertConfig;
use crate::util::{json_to_pyobject, pyobject_to_json, scouter_version};
use crate::{
    DispatchDriftConfig, DriftArgs, DriftType, FeatureMap, FileName, ProfileArgs, ProfileBaseArgs,
    ProfileRequest, PyHelperFuncs, MISSING,
};

use chrono::{DateTime, Utc};
use core::fmt::Debug;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::{VersionRequest, DEFAULT_VERSION};
use scouter_semver::VersionType;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

/// Python class for a monitoring profile
///
/// # Arguments
///
/// * `id` - The id value
/// * `center` - The center value
/// * `ucl` - The upper control limit
/// * `lcl` - The lower control limit
/// * `timestamp` - The timestamp value
///
#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct SpcFeatureDriftProfile {
    #[pyo3(get)]
    pub id: String,

    #[pyo3(get)]
    pub center: f64,

    #[pyo3(get)]
    pub one_ucl: f64,

    #[pyo3(get)]
    pub one_lcl: f64,

    #[pyo3(get)]
    pub two_ucl: f64,

    #[pyo3(get)]
    pub two_lcl: f64,

    #[pyo3(get)]
    pub three_ucl: f64,

    #[pyo3(get)]
    pub three_lcl: f64,

    #[pyo3(get)]
    pub timestamp: DateTime<Utc>,
}

/// Python class for a monitoring configuration
///
/// # Arguments
///
/// * `sample_size` - The sample size
/// * `sample` - Whether to sample data or not, Default is true
/// * `name` - The name of the model
/// * `space` - The space associated with the model
/// * `version` - The version of the model
/// * `schedule` - The cron schedule for monitoring
/// * `alert_rule` - The alerting rule to use for monitoring
///
#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct SpcDriftConfig {
    #[pyo3(get, set)]
    pub sample_size: usize,

    #[pyo3(get, set)]
    pub sample: bool,

    #[pyo3(get, set)]
    pub space: String,

    #[pyo3(get, set)]
    pub name: String,

    #[pyo3(get, set)]
    pub version: String,

    #[pyo3(get, set)]
    pub uid: String,

    #[pyo3(get, set)]
    pub alert_config: SpcAlertConfig,

    #[pyo3(get)]
    #[serde(default)]
    pub feature_map: FeatureMap,

    #[pyo3(get, set)]
    #[serde(default = "default_drift_type")]
    pub drift_type: DriftType,
}

impl Default for SpcDriftConfig {
    fn default() -> Self {
        Self {
            sample_size: 25,
            sample: true,
            space: MISSING.to_string(),
            name: MISSING.to_string(),
            uid: MISSING.to_string(),
            version: DEFAULT_VERSION.to_string(),
            alert_config: SpcAlertConfig::default(),
            feature_map: FeatureMap::default(),
            drift_type: DriftType::Spc,
        }
    }
}

fn default_drift_type() -> DriftType {
    DriftType::Spc
}

#[pymethods]
#[allow(clippy::too_many_arguments)]
impl SpcDriftConfig {
    #[new]
    #[pyo3(signature = (space=MISSING, name=MISSING, version=DEFAULT_VERSION, sample=None, sample_size=None, alert_config=None, config_path=None))]
    pub fn new(
        space: &str,
        name: &str,
        version: &str,
        sample: Option<bool>,
        sample_size: Option<usize>,
        alert_config: Option<SpcAlertConfig>,
        config_path: Option<PathBuf>,
    ) -> Result<Self, ProfileError> {
        if let Some(config_path) = config_path {
            let config = SpcDriftConfig::load_from_json_file(config_path);
            return config;
        }

        let sample = sample.unwrap_or(true);
        let sample_size = sample_size.unwrap_or(25);
        let alert_config = alert_config.unwrap_or_default();

        Ok(Self {
            sample_size,
            sample,
            name: name.to_string(),
            space: space.to_string(),
            version: version.to_string(),
            uid: MISSING.to_string(),
            alert_config,
            feature_map: FeatureMap::default(),
            drift_type: DriftType::Spc,
        })
    }

    #[staticmethod]
    pub fn load_from_json_file(path: PathBuf) -> Result<SpcDriftConfig, ProfileError> {
        // deserialize the string to a struct

        let file = std::fs::read_to_string(&path)?;

        Ok(serde_json::from_str(&file)?)
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__json__(self)
    }

    // update the arguments of the drift config
    //
    // # Arguments
    //
    // * `name` - The name of the model
    // * `space` - The space associated with the model
    // * `version` - The version of the model
    // * `sample` - Whether to sample data or not, Default is true
    // * `sample_size` - The sample size
    // * `alert_config` - The alerting configuration to use
    //
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (space=None, name=None, version=None, sample=None, sample_size=None, alert_config=None))]
    pub fn update_config_args(
        &mut self,
        space: Option<String>,
        name: Option<String>,
        version: Option<String>,
        sample: Option<bool>,
        sample_size: Option<usize>,
        alert_config: Option<SpcAlertConfig>,
    ) -> Result<(), ProfileError> {
        if name.is_some() {
            self.name = name.ok_or(TypeError::MissingNameError)?;
        }

        if space.is_some() {
            self.space = space.ok_or(TypeError::MissingSpaceError)?;
        }

        if version.is_some() {
            self.version = version.ok_or(TypeError::MissingVersionError)?;
        }

        if sample.is_some() {
            self.sample = sample.ok_or(ProfileError::MissingSampleError)?;
        }

        if sample_size.is_some() {
            self.sample_size = sample_size.ok_or(ProfileError::MissingSampleSizeError)?;
        }

        if alert_config.is_some() {
            self.alert_config = alert_config.ok_or(TypeError::MissingAlertConfigError)?;
        }

        Ok(())
    }
}

impl SpcDriftConfig {
    pub fn update_feature_map(&mut self, feature_map: FeatureMap) {
        self.feature_map = feature_map;
    }

    pub fn load_map_from_json(path: PathBuf) -> Result<HashMap<String, Value>, ProfileError> {
        // deserialize the string to a struct
        let file = std::fs::read_to_string(&path)?;
        let config = serde_json::from_str(&file)?;
        Ok(config)
    }
}

impl DispatchDriftConfig for SpcDriftConfig {
    fn get_drift_args(&self) -> DriftArgs {
        DriftArgs {
            name: self.name.clone(),
            space: self.space.clone(),
            version: self.version.clone(),
            dispatch_config: self.alert_config.dispatch_config.clone(),
        }
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct SpcDriftProfile {
    #[pyo3(get)]
    pub features: HashMap<String, SpcFeatureDriftProfile>,

    #[pyo3(get)]
    pub config: SpcDriftConfig,

    #[pyo3(get)]
    pub scouter_version: String,
}

impl SpcDriftProfile {
    pub fn new(features: HashMap<String, SpcFeatureDriftProfile>, config: SpcDriftConfig) -> Self {
        Self {
            features,
            config,
            scouter_version: scouter_version(),
        }
    }
}

#[pymethods]
impl SpcDriftProfile {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__json__(self)
    }
    #[allow(clippy::useless_conversion)]
    pub fn model_dump(&self, py: Python) -> Result<Py<PyDict>, ProfileError> {
        let json_str = serde_json::to_string(&self)?;

        let json_value: Value = serde_json::from_str(&json_str)?;

        // Create a new Python dictionary
        let dict = PyDict::new(py);

        // Convert JSON to Python dict
        json_to_pyobject(py, &json_value, &dict)?;

        // Return the Python dictionary
        Ok(dict.into())
    }

    #[staticmethod]
    pub fn model_validate(data: &Bound<'_, PyDict>) -> SpcDriftProfile {
        let json_value = pyobject_to_json(data).unwrap();

        println!("JSON Value: {:?}", json_value);

        let string = serde_json::to_string(&json_value).unwrap();
        serde_json::from_str(&string).expect("Failed to load drift profile")
    }

    #[staticmethod]
    pub fn model_validate_json(json_string: String) -> SpcDriftProfile {
        // deserialize the string to a struct
        serde_json::from_str(&json_string).expect("Failed to load monitor profile")
    }

    // Convert python dict into a drift profile
    #[pyo3(signature = (path=None))]
    pub fn save_to_json(&self, path: Option<PathBuf>) -> Result<PathBuf, ProfileError> {
        Ok(PyHelperFuncs::save_to_json(
            self,
            path,
            FileName::SpcDriftProfile.to_str(),
        )?)
    }

    #[staticmethod]
    pub fn from_file(path: PathBuf) -> Result<SpcDriftProfile, ProfileError> {
        let file = std::fs::read_to_string(&path)?;

        Ok(serde_json::from_str(&file)?)
    }

    // update the arguments of the drift config
    //
    // # Arguments
    //
    // * `name` - The name of the model
    // * `space` - The space associated with the model
    // * `version` - The version of the model
    // * `sample` - Whether to sample data or not, Default is true
    // * `sample_size` - The sample size
    // * `feature_map` - The feature map to use
    // * `alert_config` - The alerting configuration to use
    //
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (space=None, name=None, version=None, sample=None, sample_size=None, alert_config=None))]
    pub fn update_config_args(
        &mut self,
        space: Option<String>,
        name: Option<String>,
        version: Option<String>,
        sample: Option<bool>,
        sample_size: Option<usize>,
        alert_config: Option<SpcAlertConfig>,
    ) -> Result<(), ProfileError> {
        self.config
            .update_config_args(space, name, version, sample, sample_size, alert_config)
    }

    /// Create a profile request from the profile
    pub fn create_profile_request(&self) -> Result<ProfileRequest, TypeError> {
        let version: Option<String> = if self.config.version == DEFAULT_VERSION {
            None
        } else {
            Some(self.config.version.clone())
        };

        Ok(ProfileRequest {
            space: self.config.space.clone(),
            profile: self.model_dump_json(),
            drift_type: self.config.drift_type.clone(),
            version_request: VersionRequest {
                version,
                version_type: VersionType::Minor,
                pre_tag: None,
                build_tag: None,
            },
            active: false,
            deactivate_others: false,
        })
    }
}

impl ProfileBaseArgs for SpcDriftProfile {
    /// Get the base arguments for the profile (convenience method on the server)
    fn get_base_args(&self) -> ProfileArgs {
        ProfileArgs {
            name: self.config.name.clone(),
            space: self.config.space.clone(),
            version: Some(self.config.version.clone()),
            schedule: self.config.alert_config.schedule.clone(),
            scouter_version: self.scouter_version.clone(),
            drift_type: self.config.drift_type.clone(),
        }
    }

    /// Convert the struct to a serde_json::Value
    fn to_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap()
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_drift_config() {
        let mut drift_config =
            SpcDriftConfig::new(MISSING, MISSING, DEFAULT_VERSION, None, None, None, None).unwrap();
        assert_eq!(drift_config.sample_size, 25);
        assert!(drift_config.sample);
        assert_eq!(drift_config.name, "__missing__");
        assert_eq!(drift_config.space, "__missing__");
        assert_eq!(drift_config.version, "0.1.0");
        assert_eq!(drift_config.alert_config, SpcAlertConfig::default());

        // update
        drift_config
            .update_config_args(None, Some("test".to_string()), None, None, None, None)
            .unwrap();

        assert_eq!(drift_config.name, "test");
    }
}
