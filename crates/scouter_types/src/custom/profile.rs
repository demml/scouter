#![allow(clippy::useless_conversion)]
use crate::custom::alert::{CustomMetric, CustomMetricAlertConfig};
use crate::error::{ProfileError, TypeError};
use crate::util::{json_to_pyobject, pyobject_to_json, scouter_version};
use crate::{ConfigExt, ProfileRequest};
use crate::{
    DispatchDriftConfig, DriftArgs, DriftType, FileName, ProfileArgs, ProfileBaseArgs,
    PyHelperFuncs, VersionRequest, DEFAULT_VERSION, MISSING,
};
use core::fmt::Debug;
use potato_head::create_uuid7;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use scouter_semver::VersionType;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct CustomMetricDriftConfig {
    #[pyo3(get, set)]
    pub sample_size: usize,

    #[pyo3(get, set)]
    pub space: String,

    #[pyo3(get, set)]
    pub name: String,

    #[pyo3(get, set)]
    pub version: String,

    #[pyo3(get, set)]
    pub uid: String,

    #[pyo3(get, set)]
    pub alert_config: CustomMetricAlertConfig,

    #[pyo3(get, set)]
    #[serde(default = "default_drift_type")]
    pub drift_type: DriftType,
}

impl ConfigExt for CustomMetricDriftConfig {
    fn space(&self) -> &str {
        &self.space
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        &self.version
    }
}

fn default_drift_type() -> DriftType {
    DriftType::Custom
}

impl DispatchDriftConfig for CustomMetricDriftConfig {
    fn get_drift_args(&self) -> DriftArgs {
        DriftArgs {
            name: self.name.clone(),
            space: self.space.clone(),
            version: self.version.clone(),
            dispatch_config: self.alert_config.dispatch_config.clone(),
        }
    }
}

#[pymethods]
#[allow(clippy::too_many_arguments)]
impl CustomMetricDriftConfig {
    #[new]
    #[pyo3(signature = (space=MISSING, name=MISSING, version=DEFAULT_VERSION, sample_size=25, alert_config=CustomMetricAlertConfig::default(), config_path=None))]
    pub fn new(
        space: &str,
        name: &str,
        version: &str,
        sample_size: usize,
        alert_config: CustomMetricAlertConfig,
        config_path: Option<PathBuf>,
    ) -> Result<Self, ProfileError> {
        if let Some(config_path) = config_path {
            let config = CustomMetricDriftConfig::load_from_json_file(config_path)?;
            return Ok(config);
        }

        Ok(Self {
            sample_size,
            space: space.to_string(),
            name: name.to_string(),
            version: version.to_string(),
            uid: create_uuid7(),
            alert_config,
            drift_type: DriftType::Custom,
        })
    }

    #[staticmethod]
    pub fn load_from_json_file(path: PathBuf) -> Result<CustomMetricDriftConfig, ProfileError> {
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

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (space=None, name=None, version=None, uid=None, alert_config=None))]
    pub fn update_config_args(
        &mut self,
        space: Option<String>,
        name: Option<String>,
        version: Option<String>,
        uid: Option<String>,
        alert_config: Option<CustomMetricAlertConfig>,
    ) -> Result<(), TypeError> {
        if name.is_some() {
            self.name = name.ok_or(TypeError::MissingNameError)?;
        }

        if space.is_some() {
            self.space = space.ok_or(TypeError::MissingSpaceError)?;
        }

        if version.is_some() {
            self.version = version.ok_or(TypeError::MissingVersionError)?;
        }

        if alert_config.is_some() {
            self.alert_config = alert_config.ok_or(TypeError::MissingAlertConfigError)?;
        }

        if uid.is_some() {
            self.uid = uid.ok_or(TypeError::MissingUidError)?;
        }

        Ok(())
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct CustomDriftProfile {
    #[pyo3(get)]
    pub config: CustomMetricDriftConfig,

    #[pyo3(get)]
    pub metrics: HashMap<String, f64>,

    #[pyo3(get)]
    pub scouter_version: String,
}

impl Default for CustomDriftProfile {
    fn default() -> Self {
        Self {
            config: CustomMetricDriftConfig::new(
                MISSING,
                MISSING,
                DEFAULT_VERSION,
                25,
                CustomMetricAlertConfig::default(),
                None,
            )
            .unwrap(),
            metrics: HashMap::new(),
            scouter_version: scouter_version(),
        }
    }
}

#[pymethods]
impl CustomDriftProfile {
    #[new]
    #[pyo3(signature = (config, metrics))]
    pub fn new(
        mut config: CustomMetricDriftConfig,
        metrics: Vec<CustomMetric>,
    ) -> Result<Self, ProfileError> {
        if metrics.is_empty() {
            return Err(TypeError::NoMetricsError.into());
        }

        config.alert_config.set_alert_conditions(&metrics);

        let metric_vals = metrics.iter().map(|m| (m.name.clone(), m.value)).collect();

        Ok(Self {
            config,
            metrics: metric_vals,
            scouter_version: scouter_version(),
        })
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__json__(self)
    }

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

    // Convert python dict into a drift profile
    #[pyo3(signature = (path=None))]
    pub fn save_to_json(&self, path: Option<PathBuf>) -> Result<PathBuf, ProfileError> {
        Ok(PyHelperFuncs::save_to_json(
            self,
            path,
            FileName::CustomDriftProfile.to_str(),
        )?)
    }

    #[staticmethod]
    pub fn model_validate(data: &Bound<'_, PyDict>) -> CustomDriftProfile {
        let json_value = pyobject_to_json(data).unwrap();

        let string = serde_json::to_string(&json_value).unwrap();
        serde_json::from_str(&string).expect("Failed to load drift profile")
    }

    #[staticmethod]
    pub fn model_validate_json(json_string: String) -> CustomDriftProfile {
        // deserialize the string to a struct
        serde_json::from_str(&json_string).expect("Failed to load monitor profile")
    }

    #[staticmethod]
    pub fn from_file(path: PathBuf) -> Result<CustomDriftProfile, ProfileError> {
        let file = std::fs::read_to_string(&path)?;

        Ok(serde_json::from_str(&file)?)
    }

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (space=None, name=None, version=None, uid=None, alert_config=None))]
    pub fn update_config_args(
        &mut self,
        space: Option<String>,
        name: Option<String>,
        version: Option<String>,
        uid: Option<String>,
        alert_config: Option<CustomMetricAlertConfig>,
    ) -> Result<(), TypeError> {
        self.config
            .update_config_args(space, name, version, uid, alert_config)
    }

    #[getter]
    pub fn custom_metrics(&self) -> Result<Vec<CustomMetric>, ProfileError> {
        let alert_conditions = &self
            .config
            .alert_config
            .alert_conditions
            .clone()
            .ok_or(ProfileError::CustomThresholdNotSetError)?;

        Ok(self
            .metrics
            .iter()
            .map(|(name, value)| {
                // get the alert threshold for the metric
                let alert = alert_conditions
                    .get(name)
                    .ok_or(ProfileError::CustomAlertThresholdNotFound)
                    .unwrap();
                CustomMetric::new(
                    name,
                    *value,
                    alert.alert_threshold.clone(),
                    alert.alert_threshold_value,
                )
                .unwrap()
            })
            .collect())
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

impl ProfileBaseArgs for CustomDriftProfile {
    type Config = CustomMetricDriftConfig;

    fn config(&self) -> &Self::Config {
        &self.config
    }

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

    fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AlertThreshold;
    use crate::{AlertDispatchConfig, OpsGenieDispatchConfig, SlackDispatchConfig};

    #[test]
    fn test_drift_config() {
        let mut drift_config = CustomMetricDriftConfig::new(
            MISSING,
            MISSING,
            "0.1.0",
            25,
            CustomMetricAlertConfig::default(),
            None,
        )
        .unwrap();
        assert_eq!(drift_config.name, "__missing__");
        assert_eq!(drift_config.space, "__missing__");
        assert_eq!(drift_config.version, "0.1.0");
        assert_eq!(
            drift_config.alert_config.dispatch_config,
            AlertDispatchConfig::default()
        );

        let test_slack_dispatch_config = SlackDispatchConfig {
            channel: "test-channel".to_string(),
        };
        let new_alert_config = CustomMetricAlertConfig {
            schedule: "0 0 * * * *".to_string(),
            dispatch_config: AlertDispatchConfig::Slack(test_slack_dispatch_config.clone()),
            ..Default::default()
        };

        // update
        drift_config
            .update_config_args(
                None,
                Some("test".to_string()),
                None,
                None,
                Some(new_alert_config),
            )
            .unwrap();

        assert_eq!(drift_config.name, "test");
        assert_eq!(
            drift_config.alert_config.dispatch_config,
            AlertDispatchConfig::Slack(test_slack_dispatch_config)
        );
        assert_eq!(
            drift_config.alert_config.schedule,
            "0 0 * * * *".to_string()
        );
    }

    #[test]
    fn test_custom_drift_profile() {
        let alert_config = CustomMetricAlertConfig {
            schedule: "0 0 * * * *".to_string(),
            dispatch_config: AlertDispatchConfig::OpsGenie(OpsGenieDispatchConfig {
                team: "test-team".to_string(),
                priority: "P5".to_string(),
            }),
            ..Default::default()
        };

        let drift_config =
            CustomMetricDriftConfig::new("scouter", "ML", "0.1.0", 25, alert_config, None).unwrap();

        let custom_metrics = vec![
            CustomMetric::new("mae", 12.4, AlertThreshold::Above, Some(2.3)).unwrap(),
            CustomMetric::new("accuracy", 0.85, AlertThreshold::Below, None).unwrap(),
        ];

        let profile = CustomDriftProfile::new(drift_config, custom_metrics).unwrap();
        let _: Value =
            serde_json::from_str(&profile.model_dump_json()).expect("Failed to parse actual JSON");

        assert_eq!(profile.metrics.len(), 2);
        assert_eq!(profile.scouter_version, env!("CARGO_PKG_VERSION"));
        let conditions = profile.config.alert_config.alert_conditions.unwrap();
        assert_eq!(conditions["mae"].alert_threshold, AlertThreshold::Above);
        assert_eq!(conditions["mae"].alert_threshold_value, Some(2.3));
        assert_eq!(
            conditions["accuracy"].alert_threshold,
            AlertThreshold::Below
        );
        assert_eq!(conditions["accuracy"].alert_threshold_value, None);
    }
}
