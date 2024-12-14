use crate::core::cron::EveryDay;
use crate::core::dispatch::types::AlertDispatchType;
use crate::core::drift::base::{
    DispatchDriftConfig, DriftArgs, DriftType, ProfileArgs, ProfileBaseArgs, ValidateAlertConfig,
    MISSING,
};
use crate::core::error::{CustomMetricError, MonitorError, ScouterError};
use crate::core::utils::{json_to_pyobject, pyobject_to_json, FileName, ProfileFuncs};
use pyo3::types::PyDict;
use pyo3::{pyclass, pymethods, Bound, Py, PyResult, Python};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::debug;

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct CustomMetricBaseAlertConfig {
    #[pyo3(get, set)]
    pub dispatch_type: AlertDispatchType,

    #[pyo3(get, set)]
    pub dispatch_kwargs: HashMap<String, String>,

    #[pyo3(get, set)]
    pub features_to_monitor: Vec<String>,
}

#[pymethods]
impl CustomMetricBaseAlertConfig {
    #[new]
    pub fn new(
        dispatch_type: Option<AlertDispatchType>,
        dispatch_kwargs: Option<HashMap<String, String>>,
        features_to_monitor: Option<Vec<String>>,
    ) -> Self {
        let dispatch_type = dispatch_type.unwrap_or_default();
        let dispatch_kwargs = dispatch_kwargs.unwrap_or_default();
        let features_to_monitor = features_to_monitor.unwrap_or_default();

        Self {
            dispatch_type,
            dispatch_kwargs,
            features_to_monitor,
        }
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct CustomThresholdMetricAlertConfig {
    #[pyo3(get, set)]
    pub base_config: CustomMetricBaseAlertConfig,

    #[pyo3(get, set)]
    pub alert_threshold: f64,
}

#[pymethods]
impl CustomThresholdMetricAlertConfig {
    #[new]
    pub fn new(base_config: CustomMetricBaseAlertConfig, alert_threshold: f64) -> Self {
        Self {
            base_config,
            alert_threshold,
        }
    }

    #[getter]
    pub fn dispatch_type(&self) -> String {
        match self.base_config.dispatch_type {
            AlertDispatchType::Slack => "Slack".to_string(),
            AlertDispatchType::Console => "Console".to_string(),
            AlertDispatchType::OpsGenie => "OpsGenie".to_string(),
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
    OUTSIDE,
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
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct CustomComparisonMetricAlertConfig {
    #[pyo3(get, set)]
    pub base_config: CustomMetricBaseAlertConfig,

    #[pyo3(get, set)]
    pub alert_condition: AlertCondition,

    #[pyo3(get, set)]
    pub alert_boundary: Option<f64>,
}

#[pymethods]
impl CustomComparisonMetricAlertConfig {
    #[new]
    pub fn new(
        base_config: CustomMetricBaseAlertConfig,
        alert_condition: AlertCondition,
        alert_boundary: Option<f64>,
    ) -> Self {
        Self {
            base_config,
            alert_condition,
            alert_boundary,
        }
    }

    #[getter]
    pub fn dispatch_type(&self) -> String {
        match self.base_config.dispatch_type {
            AlertDispatchType::Slack => "Slack".to_string(),
            AlertDispatchType::Console => "Console".to_string(),
            AlertDispatchType::OpsGenie => "OpsGenie".to_string(),
        }
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
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

    #[pyo3(get)]
    pub threshold_metric_alert_configs: Option<HashMap<String, CustomThresholdMetricAlertConfig>>,

    #[pyo3(get)]
    pub comparison_metric_alert_configs: Option<HashMap<String, CustomComparisonMetricAlertConfig>>,

    #[pyo3(get)]
    pub drift_type: DriftType,

    #[pyo3(get, set)]
    pub schedule: String,
}

impl CustomMetricDriftConfig {
    fn set_threshold_configs(&mut self, metrics: &[CustomThresholdMetric]) {
        self.threshold_metric_alert_configs = Some(
            metrics
                .iter()
                .map(|m| (m.metric_name.clone(), m.alert_config.clone()))
                .collect(),
        );
    }

    fn set_comparison_configs(&mut self, metrics: &[CustomComparisonMetric]) {
        self.comparison_metric_alert_configs = Some(
            metrics
                .iter()
                .map(|m| (m.metric_name.clone(), m.alert_config.clone()))
                .collect(),
        );
    }
}

impl ValidateAlertConfig for CustomMetricDriftConfig {}

#[pymethods]
#[allow(clippy::too_many_arguments)]
impl CustomMetricDriftConfig {
    #[new]
    pub fn new(
        repository: Option<String>,
        name: Option<String>,
        version: Option<String>,
        schedule: Option<String>,
    ) -> Result<Self, ScouterError> {
        let name = name.unwrap_or(MISSING.to_string());
        let repository = repository.unwrap_or(MISSING.to_string());

        if name == MISSING || repository == MISSING {
            debug!("Name and repository were not provided. Defaulting to __missing__");
        }

        let version = version.unwrap_or("0.1.0".to_string());

        let schedule = Self::resolve_schedule(schedule);

        Ok(Self {
            repository,
            name,
            version,
            threshold_metric_alert_configs: None,
            comparison_metric_alert_configs: None,
            drift_type: DriftType::CUSTOM,
            schedule,
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

    // TODO look at how to update mertic configurations
    #[allow(clippy::too_many_arguments)]
    pub fn update_drift_config_args(
        &mut self,
        repository: Option<String>,
        name: Option<String>,
        version: Option<String>,
        schedule: Option<String>,
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

        if schedule.is_some() {
            self.version = schedule.ok_or(ScouterError::TypeError("schedule".to_string()))?;
        }

        Ok(())
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
    pub fn new(feature_name: String, metric_value: f64) -> Self {
        Self {
            feature_name,
            metric_value,
        }
    }
}

pub trait MetricTrait {
    fn metric_name(&self) -> &str;
    fn features(&self) -> &Vec<CustomMetricEntry>;
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomThresholdMetric {
    #[pyo3(get, set)]
    pub metric_name: String,

    #[pyo3(get, set)]
    pub features: Vec<CustomMetricEntry>,

    #[pyo3(get, set)]
    pub alert_config: CustomThresholdMetricAlertConfig,
}

impl MetricTrait for CustomThresholdMetric {
    fn metric_name(&self) -> &str {
        &self.metric_name
    }

    fn features(&self) -> &Vec<CustomMetricEntry> {
        &self.features
    }
}

#[pymethods]
impl CustomThresholdMetric {
    #[new]
    pub fn new(
        metric_name: String,
        features: Vec<CustomMetricEntry>,
        alert_config: CustomThresholdMetricAlertConfig,
    ) -> Self {
        Self {
            metric_name,
            features,
            alert_config,
        }
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
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
    pub alert_config: CustomComparisonMetricAlertConfig,
}

impl MetricTrait for CustomComparisonMetric {
    fn metric_name(&self) -> &str {
        &self.metric_name
    }

    fn features(&self) -> &Vec<CustomMetricEntry> {
        &self.features
    }
}

#[pymethods]
impl CustomComparisonMetric {
    #[new]
    pub fn new(
        metric_name: String,
        features: Vec<CustomMetricEntry>,
        alert_config: CustomComparisonMetricAlertConfig,
    ) -> Self {
        Self {
            metric_name,
            features,
            alert_config,
        }
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}

type MetricName = String;
type MetricValue = f64;
type FeatureName = String;

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomDriftProfile {
    #[pyo3(get)]
    pub config: CustomMetricDriftConfig,

    #[pyo3(get)]
    pub metrics: HashMap<MetricName, HashMap<FeatureName, MetricValue>>,

    #[pyo3(get)]
    pub scouter_version: String,
}

impl CustomDriftProfile {
    fn populate_profile_metric_hashmap<T: MetricTrait>(
        metrics_vec: &[T],
        metrics: &mut HashMap<MetricName, HashMap<FeatureName, MetricValue>>,
    ) {
        for metric in metrics_vec {
            let inner_map = metric.features().iter().map(|feature_metric_entry| {
                (
                    feature_metric_entry.feature_name.clone(),
                    feature_metric_entry.metric_value,
                )
            }).collect();

            metrics.insert(metric.metric_name().to_string(), inner_map);
        }
    }
}

#[pymethods]
impl CustomDriftProfile {
    #[new]
    pub fn new(
        mut config: CustomMetricDriftConfig,
        comparison_metrics: Option<Vec<CustomComparisonMetric>>,
        threshold_metrics: Option<Vec<CustomThresholdMetric>>,
        scouter_version: Option<String>,
    ) -> Result<Self, CustomMetricError> {
        let mut metrics: HashMap<MetricName, HashMap<FeatureName, MetricValue>> = HashMap::new();


        if let Some(comparison) = comparison_metrics.filter(|v| !v.is_empty()) {
            config.set_comparison_configs(&comparison);
            Self::populate_profile_metric_hashmap(&comparison, &mut metrics);
        }

        if let Some(threshold) = threshold_metrics.filter(|v| !v.is_empty()) {
            config.set_threshold_configs(&threshold);
            Self::populate_profile_metric_hashmap(&threshold, &mut metrics);
        }

        if metrics.is_empty() {
            return Err(CustomMetricError::NoMetricsError);
        }

        let scouter_version = scouter_version.unwrap_or(env!("CARGO_PKG_VERSION").to_string());

        Ok(Self {
            config,
            metrics,
            scouter_version,
        })
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
        let dict = PyDict::new_bound(py);

        // Convert JSON to Python dict
        json_to_pyobject(py, &json_value, dict.as_gil_ref())?;

        // Return the Python dictionary
        Ok(dict.into())
    }

    #[staticmethod]
    pub fn model_validate(py: Python, data: &Bound<'_, PyDict>) -> CustomDriftProfile {
        let json_value = pyobject_to_json(py, data.as_gil_ref()).unwrap();

        let string = serde_json::to_string(&json_value).unwrap();
        serde_json::from_str(&string).expect("Failed to load drift profile")
    }

    #[staticmethod]
    pub fn model_validate_json(json_string: String) -> CustomDriftProfile {
        // deserialize the string to a struct
        serde_json::from_str(&json_string).expect("Failed to load monitor profile")
    }

    // Convert python dict into a drift profile
    pub fn save_to_json(&self, path: Option<PathBuf>) -> Result<(), ScouterError> {
        ProfileFuncs::save_to_json(self, path, FileName::Profile.to_str())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_drift_config_args(
        &mut self,
        repository: Option<String>,
        name: Option<String>,
        version: Option<String>,
        schedule: Option<String>,
    ) -> Result<(), ScouterError> {
        self.config
            .update_drift_config_args(repository, name, version, schedule)
    }
}

impl ProfileBaseArgs for CustomDriftProfile {
    /// Get the base arguments for the profile (convenience method on the server)
    fn get_base_args(&self) -> ProfileArgs {
        ProfileArgs {
            name: self.config.name.clone(),
            repository: self.config.repository.clone(),
            version: self.config.version.clone(),
            schedule: self.config.schedule.clone(),
            scouter_version: self.scouter_version.clone(),
            drift_type: self.config.drift_type.clone(),
        }
    }

    /// Convert the struct to a serde_json::Value
    fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
}
