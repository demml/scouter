use crate::core::cron::EveryDay;
use crate::core::dispatch::types::AlertDispatchType;
use crate::core::drift::base::{
    DispatchAlertDescription, DispatchDriftConfig, DriftArgs, DriftType, ProfileArgs,
    ProfileBaseArgs, ValidateAlertConfig, MISSING,
};
use crate::core::error::{CustomMetricError, ScouterError};
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
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomMetricAlertCondition {
    #[pyo3(get, set)]
    pub alert_condition: AlertCondition,

    #[pyo3(get, set)]
    pub alert_boundary: Option<f64>,
}

#[pymethods]
#[allow(clippy::too_many_arguments)]
impl CustomMetricAlertCondition {
    #[new]
    pub fn new(
        alert_condition: AlertCondition,
        alert_boundary: Option<f64>,
    ) -> Result<Self, ScouterError> {
        Ok(Self {
            alert_condition,
            alert_boundary,
        })
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomMetricAlertConfig {
    pub dispatch_type: AlertDispatchType,

    #[pyo3(get, set)]
    pub schedule: String,

    #[pyo3(get, set)]
    pub dispatch_kwargs: HashMap<String, String>,

    #[pyo3(get, set)]
    pub alert_conditions: Option<HashMap<String, CustomMetricAlertCondition>>,
}

impl CustomMetricAlertConfig {
    fn set_alert_conditions(&mut self, metrics: &[CustomMetric]) {
        self.alert_conditions = Some(
            metrics
                .iter()
                .map(|m| {
                    (
                        m.name.clone(),
                        CustomMetricAlertCondition {
                            alert_condition: m.alert_condition.clone(),
                            alert_boundary: m.alert_boundary,
                        },
                    )
                })
                .collect(),
        );
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
    ) -> Self {
        let schedule = Self::resolve_schedule(schedule);
        let dispatch_type = dispatch_type.unwrap_or_default();
        let dispatch_kwargs = dispatch_kwargs.unwrap_or_default();

        Self {
            dispatch_type,
            schedule,
            dispatch_kwargs,
            alert_conditions: None,
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

impl Default for CustomMetricAlertConfig {
    fn default() -> CustomMetricAlertConfig {
        Self {
            dispatch_type: AlertDispatchType::default(),
            schedule: EveryDay::new().cron,
            dispatch_kwargs: HashMap::new(),
            alert_conditions: None,
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

    #[pyo3(get)]
    pub drift_type: DriftType,
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
        let name = name.unwrap_or(MISSING.to_string());
        let repository = repository.unwrap_or(MISSING.to_string());

        if name == MISSING || repository == MISSING {
            debug!("Name and repository were not provided. Defaulting to __missing__");
        }

        let version = version.unwrap_or("0.1.0".to_string());

        if let Some(config_path) = config_path {
            let config = CustomMetricDriftConfig::load_from_json_file(config_path);
            return config;
        }

        let alert_config = alert_config.unwrap_or_default();

        Ok(Self {
            repository,
            name,
            version,
            alert_config,
            drift_type: DriftType::CUSTOM,
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

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomMetric {
    #[pyo3(get, set)]
    pub name: String,

    #[pyo3(get, set)]
    pub value: f64,

    #[pyo3(get, set)]
    pub alert_boundary: Option<f64>,

    #[pyo3(get, set)]
    pub alert_condition: AlertCondition,
}

#[pymethods]
impl CustomMetric {
    #[new]
    pub fn new(
        name: String,
        value: f64,
        alert_condition: AlertCondition,
        alert_boundary: Option<f64>,
    ) -> Self {
        Self {
            name: name.to_lowercase(),
            value,
            alert_boundary,
            alert_condition,
        }
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomDriftProfile {
    #[pyo3(get)]
    pub config: CustomMetricDriftConfig,

    #[pyo3(get)]
    pub metrics: HashMap<String, f64>,

    #[pyo3(get)]
    pub scouter_version: String,
}

#[pymethods]
impl CustomDriftProfile {
    #[new]
    pub fn new(
        mut config: CustomMetricDriftConfig,
        metrics: Vec<CustomMetric>,
        scouter_version: Option<String>,
    ) -> Result<Self, CustomMetricError> {
        if metrics.is_empty() {
            return Err(CustomMetricError::NoMetricsError);
        }

        config.alert_config.set_alert_conditions(&metrics);

        let metrics = metrics.iter().map(|m| (m.name.clone(), m.value)).collect();

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

    // Convert python dict into a drift profile
    pub fn save_to_json(&self, path: Option<PathBuf>) -> Result<(), ScouterError> {
        ProfileFuncs::save_to_json(self, path, FileName::Profile.to_str())
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

    #[allow(clippy::too_many_arguments)]
    pub fn update_config_args(
        &mut self,
        repository: Option<String>,
        name: Option<String>,
        version: Option<String>,
        alert_config: Option<CustomMetricAlertConfig>,
    ) -> Result<(), ScouterError> {
        self.config
            .update_config_args(repository, name, version, alert_config)
    }

    #[getter]
    pub fn custom_metrics(&self) -> Vec<CustomMetric> {
        let alert_conditions = self.config.alert_config.alert_conditions.as_ref().unwrap();

        self.metrics
            .iter()
            .map(|(name, value)| {
                let condition = alert_conditions.get(name).unwrap();
                CustomMetric {
                    name: name.clone(),
                    value: *value,
                    alert_boundary: condition.alert_boundary,
                    alert_condition: condition.alert_condition.clone(),
                }
            })
            .collect()
    }
}

impl ProfileBaseArgs for CustomDriftProfile {
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

    fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
}

pub struct ComparisonMetricAlert {
    pub metric_name: String,
    pub training_metric_value: f64,
    pub observed_metric_value: f64,
    pub alert_boundary: Option<f64>,
    pub alert_condition: AlertCondition,
}

impl ComparisonMetricAlert {
    fn alert_description_header(&self) -> String {
        let below_threshold = |boundary: Option<f64>| match boundary {
            Some(b) => format!(
                "The metric value has dropped below the threshold (initial value - {}) for {}",
                b, self.metric_name
            ),
            None => format!(
                "The metric value has dropped below the initial value for {}",
                self.metric_name
            ),
        };

        let above_threshold = |boundary: Option<f64>| match boundary {
            Some(b) => format!(
                "The metric value has increased beyond the threshold (initial value + {}) for {}",
                b, self.metric_name
            ),
            None => format!(
                "The metric value has increased beyond the initial value for {}",
                self.metric_name
            ),
        };

        let outside_threshold = |boundary: Option<f64>| match boundary {
            Some(b) => format!(
                "The metric value has fallen outside the threshold (initial value ± {}) for {}",
                b, self.metric_name
            ),
            None => format!(
                "The metric value has fallen outside the initial value for {}",
                self.metric_name
            ),
        };

        match self.alert_condition {
            AlertCondition::BELOW => below_threshold(self.alert_boundary),
            AlertCondition::ABOVE => above_threshold(self.alert_boundary),
            AlertCondition::OUTSIDE => outside_threshold(self.alert_boundary),
        }
    }
}

impl DispatchAlertDescription for ComparisonMetricAlert {
    fn create_alert_description(&self, dispatch_type: AlertDispatchType) -> String {
        let mut alert_description = String::new();
        let header = format!("{}\n", self.alert_description_header());
        alert_description.push_str(&header);

        let current_metric = format!("Current Metric Value: {}\n", self.observed_metric_value);
        let historical_metric =
            format!("Historical Metric Value: {}\n", self.training_metric_value);

        let feature_name = match dispatch_type {
            AlertDispatchType::Console | AlertDispatchType::OpsGenie => {
                format!("{:indent$}{}: \n", "", &self.metric_name, indent = 4)
            }
            AlertDispatchType::Slack => format!("{}: \n", &self.metric_name),
        };

        alert_description.push_str(&feature_name);
        alert_description.push_str(&current_metric);
        alert_description.push_str(&historical_metric);

        alert_description
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomMetricServerRecord {
    #[pyo3(get)]
    pub created_at: chrono::NaiveDateTime,

    #[pyo3(get)]
    pub repository: String,

    #[pyo3(get)]
    pub name: String,

    #[pyo3(get)]
    pub version: String,

    #[pyo3(get)]
    pub metric: String,

    #[pyo3(get)]
    pub value: f64,
}

#[pymethods]
impl CustomMetricServerRecord {
    #[new]
    pub fn new(
        repository: String,
        name: String,
        version: String,
        metric: String,
        value: f64,
    ) -> Self {
        Self {
            created_at: chrono::Utc::now().naive_utc(),
            name,
            repository,
            version,
            metric,
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
        record.insert("metric".to_string(), self.metric.clone());
        record.insert("value".to_string(), self.value.to_string());
        record
    }
}
