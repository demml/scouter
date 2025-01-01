use crate::{ FeatureMap,ProfileFuncs, DriftType, FileName, DriftArgs, DispatchDriftConfig, MISSING, ProfileBaseArgs, ProfileArgs};
use scouter_error::ScouterError;
use crate::spc::alert::SpcAlertConfig;
use crate::util::{json_to_pyobject, pyobject_to_json};
use core::fmt::Debug;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::debug;


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
#[derive(Debug, Serialize, Deserialize, Clone)]
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
    pub timestamp: chrono::NaiveDateTime,
}

/// Python class for a monitoring configuration
///
/// # Arguments
///
/// * `sample_size` - The sample size
/// * `sample` - Whether to sample data or not, Default is true
/// * `name` - The name of the model
/// * `repository` - The repository associated with the model
/// * `version` - The version of the model
/// * `schedule` - The cron schedule for monitoring
/// * `alert_rule` - The alerting rule to use for monitoring
///
#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpcDriftConfig {
    #[pyo3(get, set)]
    pub sample_size: usize,

    #[pyo3(get, set)]
    pub sample: bool,

    #[pyo3(get, set)]
    pub repository: String,

    #[pyo3(get, set)]
    pub name: String,

    #[pyo3(get, set)]
    pub version: String,

    #[pyo3(get, set)]
    pub alert_config: SpcAlertConfig,

    #[pyo3(get, set)]
    pub feature_map: Option<FeatureMap>,

    #[pyo3(get, set)]
    pub targets: Vec<String>,

    #[pyo3(get, set)]
    pub drift_type: DriftType,
}

#[pymethods]
#[allow(clippy::too_many_arguments)]
impl SpcDriftConfig {
    #[new]
    #[pyo3(signature = (repository=None, name=None, version=None, sample=None, sample_size=None, feature_map=None, targets=None, alert_config=None, config_path=None))]
    pub fn new(
        repository: Option<String>,
        name: Option<String>,
        version: Option<String>,
        sample: Option<bool>,
        sample_size: Option<usize>,
        feature_map: Option<FeatureMap>,
        targets: Option<Vec<String>>,
        alert_config: Option<SpcAlertConfig>,
        config_path: Option<PathBuf>,
    ) -> Result<Self, ScouterError> {
        if let Some(config_path) = config_path {
            let config = SpcDriftConfig::load_from_json_file(config_path);
            return config;
        }

        let name = name.unwrap_or(MISSING.to_string());
        let repository = repository.unwrap_or(MISSING.to_string());

        if name == MISSING || repository == MISSING {
            debug!("Name and repository were not provided. Defaulting to __missing__");
        }

        let sample = sample.unwrap_or(true);
        let sample_size = sample_size.unwrap_or(25);
        let version = version.unwrap_or("0.1.0".to_string());
        let targets = targets.unwrap_or_default();
        let alert_config = alert_config.unwrap_or_default();

        Ok(Self {
            sample_size,
            sample,
            name,
            repository,
            version,
            alert_config,
            feature_map,
            targets,
            drift_type: DriftType::Spc,
        })
    }

    pub fn update_feature_map(&mut self, feature_map: FeatureMap) {
        self.feature_map = Some(feature_map);
    }

    #[staticmethod]
    pub fn load_from_json_file(path: PathBuf) -> Result<SpcDriftConfig, ScouterError> {
        // deserialize the string to a struct

        let file = std::fs::read_to_string(&path).map_err(|_| ScouterError::ReadError)?;

        serde_json::from_str(&file).map_err(|_| ScouterError::DeSerializeError)
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__json__(self)
    }

    // update the arguments of the drift config
    //
    // # Arguments
    //
    // * `name` - The name of the model
    // * `repository` - The repository associated with the model
    // * `version` - The version of the model
    // * `sample` - Whether to sample data or not, Default is true
    // * `sample_size` - The sample size
    // * `feature_map` - The feature map to use
    // * `targets` - The targets to monitor
    // * `alert_config` - The alerting configuration to use
    //
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (repository=None, name=None, version=None, sample=None, sample_size=None, feature_map=None, targets=None, alert_config=None))]
    pub fn update_config_args(
        &mut self,
        repository: Option<String>,
        name: Option<String>,
        version: Option<String>,
        sample: Option<bool>,
        sample_size: Option<usize>,
        feature_map: Option<FeatureMap>,
        targets: Option<Vec<String>>,
        alert_config: Option<SpcAlertConfig>,
    ) -> Result<(), ScouterError> {
        if name.is_some() {
            self.name = name.ok_or(ScouterError::TypeError("name".to_string()))?;
        }

        if repository.is_some() {
            self.repository =
                repository.ok_or(ScouterError::TypeError("repository".to_string()))?;
        }

        if version.is_some() {
            self.version = version.ok_or(ScouterError::TypeError("version".to_string()))?;
        }

        if sample.is_some() {
            self.sample = sample.ok_or(ScouterError::TypeError("sample".to_string()))?;
        }

        if sample_size.is_some() {
            self.sample_size =
                sample_size.ok_or(ScouterError::TypeError("sample size".to_string()))?;
        }

        if feature_map.is_some() {
            self.feature_map = feature_map;
        }

        if targets.is_some() {
            self.targets = targets.ok_or(ScouterError::TypeError("targets".to_string()))?;
        }

        if alert_config.is_some() {
            self.alert_config =
                alert_config.ok_or(ScouterError::TypeError("alert_config".to_string()))?;
        }

        Ok(())
    }
}

impl SpcDriftConfig {
    pub fn load_map_from_json(path: PathBuf) -> Result<HashMap<String, Value>, ScouterError> {
        // deserialize the string to a struct
        let file = std::fs::read_to_string(&path).map_err(|_| ScouterError::ReadError)?;
        let config = serde_json::from_str(&file).map_err(|_| ScouterError::DeSerializeError)?;
        Ok(config)
    }
}

impl DispatchDriftConfig for SpcDriftConfig {
    fn get_drift_args(&self) -> DriftArgs {
        DriftArgs {
            name: self.name.clone(),
            repository: self.repository.clone(),
            version: self.version.clone(),
            dispatch_type: self.alert_config.dispatch_type.clone(),
        }
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpcDriftProfile {
    #[pyo3(get, set)]
    pub features: HashMap<String, SpcFeatureDriftProfile>,

    #[pyo3(get, set)]
    pub config: SpcDriftConfig,

    #[pyo3(get, set)]
    pub scouter_version: String,
}

#[pymethods]
impl SpcDriftProfile {
    #[new]
    #[pyo3(signature = (features, config, scouter_version=None))]
    pub fn new(
        features: HashMap<String, SpcFeatureDriftProfile>,
        config: SpcDriftConfig,
        scouter_version: Option<String>,
    ) -> Self {
        let scouter_version = scouter_version.unwrap_or(env!("CARGO_PKG_VERSION").to_string());
        Self {
            features,
            config,
            scouter_version,
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

    pub fn model_dump(&self, py: Python) -> PyResult<Py<PyDict>> {
        let json_str = serde_json::to_string(&self).map_err(|_| ScouterError::SerializeError)?;

        let json_value: Value =
            serde_json::from_str(&json_str).map_err(|_| ScouterError::DeSerializeError)?;

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
    pub fn save_to_json(&self, path: Option<PathBuf>) -> Result<(), ScouterError> {
        ProfileFuncs::save_to_json(self, path, FileName::Profile.to_str())
    }

    // update the arguments of the drift config
    //
    // # Arguments
    //
    // * `name` - The name of the model
    // * `repository` - The repository associated with the model
    // * `version` - The version of the model
    // * `sample` - Whether to sample data or not, Default is true
    // * `sample_size` - The sample size
    // * `feature_map` - The feature map to use
    // * `targets` - The targets to monitor
    // * `alert_config` - The alerting configuration to use
    //
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (repository=None, name=None, version=None, sample=None, sample_size=None, feature_map=None, targets=None, alert_config=None))]
    pub fn update_config_args(
        &mut self,
        repository: Option<String>,
        name: Option<String>,
        version: Option<String>,
        sample: Option<bool>,
        sample_size: Option<usize>,
        feature_map: Option<FeatureMap>,
        targets: Option<Vec<String>>,
        alert_config: Option<SpcAlertConfig>,
    ) -> Result<(), ScouterError> {
        self.config.update_config_args(
            repository,
            name,
            version,
            sample,
            sample_size,
            feature_map,
            targets,
            alert_config,
        )
    }
}

impl ProfileBaseArgs for SpcDriftProfile {
    /// Get the base arguments for the profile (convenience method on the server)
    fn get_base_args(&self) -> ProfileArgs {
        ProfileArgs {
            name: self.config.name.clone(),
            repository: self.config.repository.clone(),
            version: self.config.version.clone(),
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
            SpcDriftConfig::new(None, None, None, None, None, None, None, None, None).unwrap();
        assert_eq!(drift_config.sample_size, 25);
        assert!(drift_config.sample);
        assert_eq!(drift_config.name, "__missing__");
        assert_eq!(drift_config.repository, "__missing__");
        assert_eq!(drift_config.version, "0.1.0");
        assert_eq!(drift_config.targets.len(), 0);
        assert_eq!(drift_config.alert_config, SpcAlertConfig::default());

        // update
        drift_config
            .update_config_args(
                None,
                Some("test".to_string()),
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();

        assert_eq!(drift_config.name, "test");
    }

}