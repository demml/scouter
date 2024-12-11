use crate::core::cron::EveryDay;
use crate::core::dispatch::types::AlertDispatchType;
use crate::core::drift::base::{DispatchDriftConfig, DriftArgs, DriftType, ProfileArgs, ProfileBaseArgs, ValidateAlertConfig, MISSING};
use crate::core::error::{MonitorError, ScouterError, UserDefinitionError};
use crate::core::utils::{json_to_pyobject, pyobject_to_json, FileName, ProfileFuncs};
use pyo3::{pyclass, pymethods, Bound, Py, PyResult, Python};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use pyo3::types::PyDict;
use serde_json::Value;
use tracing::debug;

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct CustomMetricAlertConfig {
    #[pyo3(set)]
    pub dispatch_type: AlertDispatchType,

    #[pyo3(get, set)]
    pub schedule: String,

    #[pyo3(get, set)]
    pub dispatch_kwargs: HashMap<String, String>,

    #[pyo3(get)]
    pub metric_thresholds: HashMap<String, f64>,

    #[pyo3(get, set)]
    pub features_to_monitor: Vec<String>,
}

impl Default for CustomMetricAlertConfig {
    fn default() -> Self {
        Self {
            dispatch_type: AlertDispatchType::default(),
            schedule: EveryDay::new().cron,
            dispatch_kwargs: HashMap::new(),
            metric_thresholds: HashMap::new(),
            features_to_monitor: Vec::new(),
        }
    }
}

impl ValidateAlertConfig for CustomMetricAlertConfig {}

#[pymethods]
impl CustomMetricAlertConfig {
    #[new]
    pub fn new(
        dispatch_type: Option<AlertDispatchType>,
        schedule: Option<String>,
        dispatch_kwargs: Option<HashMap<String, String>>,
        metric_thresholds: Option<HashMap<String, f64>>,
        features_to_monitor: Option<Vec<String>>,
    ) -> Self {
        let schedule = Self::resolve_schedule(schedule);
        let dispatch_type = dispatch_type.unwrap_or_default();
        let dispatch_kwargs = dispatch_kwargs.unwrap_or_default();
        let metric_thresholds = metric_thresholds.unwrap_or_default();
        let features_to_monitor = features_to_monitor.unwrap_or_default();

        Self {
            dispatch_type,
            schedule,
            dispatch_kwargs,
            metric_thresholds,
            features_to_monitor
        }
    }

    #[getter]
    pub fn dispatch_type(&self) -> String {
        match self.dispatch_type {
            AlertDispatchType::Slack => "Slack".to_string(),
            AlertDispatchType::Console => "Console".to_string(),
            AlertDispatchType::OpsGenie => "OpsGenie".to_string(),
        }
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomMetricDriftConfig {
    #[pyo3(get, set)]
    pub repository: String,

    #[pyo3(get, set)]
    pub name: String,

    #[pyo3(get, set)]
    pub version: String,

    #[pyo3(get, set)]
    pub alert_config: CustomMetricAlertConfig,

    #[pyo3(get, set)]
    pub drift_type: DriftType,
}

#[pymethods]
#[allow(clippy::too_many_arguments)]
impl CustomMetricDriftConfig {
    #[new]
    pub fn new(
        repository: Option<String>,
        name: Option<String>,
        version: Option<String>,
        alert_config: Option<CustomMetricAlertConfig>,
        config_path: Option<PathBuf>,
    ) -> Result<Self, ScouterError> {
        if let Some(config_path) = config_path {
            let config = CustomMetricDriftConfig::load_from_json_file(config_path);
            return config;
        }

        let name = name.unwrap_or(MISSING.to_string());
        let repository = repository.unwrap_or(MISSING.to_string());

        if name == MISSING || repository == MISSING {
            debug!("Name and repository were not provided. Defaulting to __missing__");
        }

        let version = version.unwrap_or("0.1.0".to_string());
        let alert_config = alert_config.unwrap_or_default();

        Ok(Self {
            name,
            repository,
            version,
            alert_config,
            drift_type: DriftType::Custom,
        })
    }

    #[staticmethod]
    pub fn load_from_json_file(path: PathBuf) -> Result<CustomMetricDriftConfig, ScouterError> {
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

    #[allow(clippy::too_many_arguments)]
    pub fn update_config_args(
        &mut self,
        repository: Option<String>,
        name: Option<String>,
        version: Option<String>,
        alert_config: Option<CustomMetricAlertConfig>,
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

        if alert_config.is_some() {
            self.alert_config =
                alert_config.ok_or(ScouterError::TypeError("alert_config".to_string()))?;
        }

        Ok(())
    }
}

impl DispatchDriftConfig for CustomMetricDriftConfig {
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
pub struct CustomMetricEntry {
    #[pyo3(get, set)]
    pub feature_name: String,

    #[pyo3(get, set)]
    pub metric_value: f64,
}

#[pymethods]
impl CustomMetricEntry {
    #[new]
    pub fn new(
        feature_name: String,
        metric_value: f64,
    ) -> Self {
        Self {
            feature_name,
            metric_value
        }
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}


#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomThresholdMetric {
    #[pyo3(get, set)]
    pub metric_name: String,

    #[pyo3(get, set)]
    pub features: Vec<CustomMetricEntry>,

    #[pyo3(get, set)]
    pub alert_threshold: f64,
}

#[pymethods]
impl CustomThresholdMetric {
    #[new]
    pub fn new(
        metric_name: String,
        features: Vec<CustomMetricEntry>,
        alert_threshold: f64,
    ) -> Self {

        Self {
            metric_name,
            features,
            alert_threshold
        }
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}


#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum AlertCondition {
    BELOW,
    ABOVE,
    OUTSIDE
}

#[pymethods]
impl AlertCondition {
    #[staticmethod]
    pub fn from_value(value: &str) -> Option<Self> {
        match value {
            "BELOW" => Some(AlertCondition::BELOW),
            "ABOVE" => Some(AlertCondition::ABOVE),
            "OUTSIDE" => Some(AlertCondition::OUTSIDE),
            _ => None,
        }
    }

    #[getter]
    pub fn value(&self) -> &str {
        match self {
            AlertCondition::BELOW => "BELOW",
            AlertCondition::ABOVE => "ABOVE",
            AlertCondition::OUTSIDE => "OUTSIDE",
        }
    }
}



#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomComparisonMetric {
    #[pyo3(get, set)]
    pub metric_name: String,

    #[pyo3(get, set)]
    pub features: Vec<CustomMetricEntry>,

    #[pyo3(get, set)]
    pub alert_condition: AlertCondition,

    #[pyo3(get, set)]
    pub alert_boundary: Option<f64>,
}

#[pymethods]
impl CustomComparisonMetric {
    #[new]
    pub fn new(
        metric_name: String,
        features: Vec<CustomMetricEntry>,
        alert_condition: AlertCondition,
        alert_boundary: Option<f64>,
    ) -> Self {
        Self {
            metric_name,
            features,
            alert_condition,
            alert_boundary
        }
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}

// pub fn apple(comparison_metrics: Vec<CustomComparisonMetric>, threshold_metrics: Vec<CustomThresholdMetric>) {
//     for metric in comparison_metrics {
//         let threshold = cmp.alert_boundary;
//         let alert_condition = cmp.alert_condition;
//         let metric_name = cmp.metric_name;
//         // pull metric_name
//         // pull features with metric
//         // computer mean of metric for feature
//         for grop in cmp.features {
//             let feature = grop.feature_name;
//             let value = grop.metric_value;
//             // compare against training val
//         //     if exceeds send alert
//         }
//     }
// }

pub type MetricName = String;
pub type FeatureName = String;

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomDriftProfile {
    #[pyo3(get)]
    pub metrics: HashMap<MetricName, HashMap<FeatureName, f64>>,

    #[pyo3(get)]
    pub config: CustomMetricDriftConfig,

    #[pyo3(get)]
    pub scouter_version: String,
}

#[pymethods]
impl CustomDriftProfile {
    #[new]
    pub fn new(
        config: CustomMetricDriftConfig,
        comparison_metrics: Vec<CustomComparisonMetric>,
        threshold_metrics: Vec<CustomThresholdMetric>,
        scouter_version: Option<String>,
    ) -> Self {
        let scouter_version = scouter_version.unwrap_or(env!("CARGO_PKG_VERSION").to_string());

        Self {
            config,
            metrics: HashMap::new(),
            scouter_version
        }
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}


impl ProfileBaseArgs for CustomDriftProfile {
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
    fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
}
