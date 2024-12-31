use crate::core::cron::EveryDay;
use crate::core::dispatch::types::AlertDispatchType;
use crate::core::drift::base::{
    DispatchAlertDescription, DispatchDriftConfig, DriftArgs, DriftType, ProfileArgs,
    ProfileBaseArgs, RecordType, ValidateAlertConfig, MISSING,
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

#[pyclass(eq)]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum AlertThreshold {
    Below,
    Above,
    Outside,
}

#[pymethods]
impl AlertThreshold {
    #[staticmethod]
    pub fn from_value(value: &str) -> Option<Self> {
        match value.to_lowercase().as_str() {
            "below" => Some(AlertThreshold::Below),
            "above" => Some(AlertThreshold::Above),
            "outside" => Some(AlertThreshold::Outside),
            _ => None,
        }
    }

    pub fn __str__(&self) -> String {
        match self {
            AlertThreshold::Below => "Below".to_string(),
            AlertThreshold::Above => "Above".to_string(),
            AlertThreshold::Outside => "Outside".to_string(),
        }
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomMetricAlertCondition {
    #[pyo3(get, set)]
    pub alert_threshold: AlertThreshold,

    #[pyo3(get, set)]
    pub alert_threshold_value: Option<f64>,
}

#[pymethods]
#[allow(clippy::too_many_arguments)]
impl CustomMetricAlertCondition {
    #[new]
    #[pyo3(signature = (alert_threshold, alert_threshold_value=None))]
    pub fn new(
        alert_threshold: AlertThreshold,
        alert_threshold_value: Option<f64>,
    ) -> Result<Self, ScouterError> {
        Ok(Self {
            alert_threshold,
            alert_threshold_value,
        })
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
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
                .map(|m| (m.name.clone(), m.alert_condition.clone()))
                .collect(),
        );
    }
}

impl ValidateAlertConfig for CustomMetricAlertConfig {}

#[pymethods]
impl CustomMetricAlertConfig {
    #[new]
    #[pyo3(signature = (dispatch_type=None, schedule=None, dispatch_kwargs=None))]
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
    #[pyo3(signature = (repository=None, name=None, version=None, alert_config=None, config_path=None))]
    pub fn new(
        repository: Option<String>,
        name: Option<String>,
        version: Option<String>,
        alert_config: Option<CustomMetricAlertConfig>,
        config_path: Option<PathBuf>,
    ) -> Result<Self, CustomMetricError> {
        let name = name.unwrap_or(MISSING.to_string());
        let repository = repository.unwrap_or(MISSING.to_string());

        if name == MISSING || repository == MISSING {
            debug!("Name and repository were not provided. Defaulting to __missing__");
        }

        let version = version.unwrap_or("0.1.0".to_string());

        if let Some(config_path) = config_path {
            let config = CustomMetricDriftConfig::load_from_json_file(config_path)
                .map_err(|e| CustomMetricError::Error(e.to_string()));

            return config;
        }

        let alert_config = alert_config.unwrap_or_default();

        Ok(Self {
            repository,
            name,
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
    #[pyo3(signature = (repository=None, name=None, version=None, alert_config=None))]
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
    pub alert_condition: CustomMetricAlertCondition,
}

#[pymethods]
impl CustomMetric {
    #[new]
    #[pyo3(signature = (name, value, alert_threshold, alert_threshold_value=None))]
    pub fn new(
        name: String,
        value: f64,
        alert_threshold: AlertThreshold,
        alert_threshold_value: Option<f64>,
    ) -> Result<Self, CustomMetricError> {
        let custom_condition =
            CustomMetricAlertCondition::new(alert_threshold, alert_threshold_value)
                .map_err(|e| CustomMetricError::Error(e.to_string()))?;

        Ok(Self {
            name: name.to_lowercase(),
            value,
            alert_condition: custom_condition,
        })
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }

    #[getter]
    pub fn alert_threshold(&self) -> AlertThreshold {
        self.alert_condition.alert_threshold.clone()
    }

    #[getter]
    pub fn alert_threshold_value(&self) -> Option<f64> {
        self.alert_condition.alert_threshold_value
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

    #[pyo3(get)]
    pub custom_metrics: Vec<CustomMetric>,
}

#[pymethods]
impl CustomDriftProfile {
    #[new]
    #[pyo3(signature = (config, metrics, scouter_version=None))]
    pub fn new(
        mut config: CustomMetricDriftConfig,
        metrics: Vec<CustomMetric>,
        scouter_version: Option<String>,
    ) -> Result<Self, CustomMetricError> {
        if metrics.is_empty() {
            return Err(CustomMetricError::NoMetricsError);
        }

        config.alert_config.set_alert_conditions(&metrics);

        let metric_vals = metrics.iter().map(|m| (m.name.clone(), m.value)).collect();

        let scouter_version = scouter_version.unwrap_or(env!("CARGO_PKG_VERSION").to_string());

        Ok(Self {
            config,
            metrics: metric_vals,
            scouter_version,
            custom_metrics: metrics,
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
        let dict = PyDict::new(py);

        // Convert JSON to Python dict
        json_to_pyobject(py, &json_value, &dict)?;

        // Return the Python dictionary
        Ok(dict.into())
    }

    // Convert python dict into a drift profile
    #[pyo3(signature = (path=None))]
    pub fn save_to_json(&self, path: Option<PathBuf>) -> Result<(), ScouterError> {
        ProfileFuncs::save_to_json(self, path, FileName::Profile.to_str())
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

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (repository=None, name=None, version=None, alert_config=None))]
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
    pub alert_threshold_value: Option<f64>,
    pub alert_threshold: AlertThreshold,
}

impl ComparisonMetricAlert {
    fn alert_description_header(&self) -> String {
        let below_threshold = |boundary: Option<f64>| match boundary {
            Some(b) => format!(
                "The observed {} metric value has dropped below the threshold (initial value - {})",
                self.metric_name, b
            ),
            None => format!(
                "The {} metric value has dropped below the initial value",
                self.metric_name
            ),
        };

        let above_threshold = |boundary: Option<f64>| match boundary {
            Some(b) => format!(
                "The {} metric value has increased beyond the threshold (initial value + {})",
                self.metric_name, b
            ),
            None => format!(
                "The {} metric value has increased beyond the initial value",
                self.metric_name
            ),
        };

        let outside_threshold = |boundary: Option<f64>| match boundary {
            Some(b) => format!(
                "The {} metric value has fallen outside the threshold (initial value ± {})",
                self.metric_name, b,
            ),
            None => format!(
                "The metric value has fallen outside the initial value for {}",
                self.metric_name
            ),
        };

        match self.alert_threshold {
            AlertThreshold::Below => below_threshold(self.alert_threshold_value),
            AlertThreshold::Above => above_threshold(self.alert_threshold_value),
            AlertThreshold::Outside => outside_threshold(self.alert_threshold_value),
        }
    }
}

impl DispatchAlertDescription for ComparisonMetricAlert {
    // TODO make pretty per dispatch type
    fn create_alert_description(&self, _dispatch_type: AlertDispatchType) -> String {
        let mut alert_description = String::new();
        let header = format!("{}\n", self.alert_description_header());
        alert_description.push_str(&header);

        let current_metric = format!("Current Metric Value: {}\n", self.observed_metric_value);
        let historical_metric = format!("Initial Metric Value: {}\n", self.training_metric_value);

        alert_description.push_str(&historical_metric);
        alert_description.push_str(&current_metric);

        alert_description
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_config() {
        //test console alert config
        let dispatch_type = AlertDispatchType::OpsGenie;
        let schedule = "0 0 * * * *".to_string();
        let mut alert_config =
            CustomMetricAlertConfig::new(Some(dispatch_type), Some(schedule), None);
        assert_eq!(alert_config.dispatch_type(), "OpsGenie");

        let custom_metrics = vec![
            CustomMetric::new("mae".to_string(), 12.4, AlertThreshold::Above, Some(2.3)).unwrap(),
            CustomMetric::new("accuracy".to_string(), 0.85, AlertThreshold::Below, None).unwrap(),
        ];

        alert_config.set_alert_conditions(&custom_metrics);

        if let Some(alert_conditions) = alert_config.alert_conditions.as_ref() {
            assert_eq!(
                alert_conditions["mae"].alert_threshold,
                AlertThreshold::Above
            );
            assert_eq!(alert_conditions["mae"].alert_threshold_value, Some(2.3));
            assert_eq!(
                alert_conditions["accuracy"].alert_threshold,
                AlertThreshold::Below
            );
            assert_eq!(alert_conditions["accuracy"].alert_threshold_value, None);
        } else {
            panic!("alert_conditions should not be None");
        }
    }

    #[test]
    fn test_drift_config() {
        let mut drift_config = CustomMetricDriftConfig::new(None, None, None, None, None).unwrap();
        assert_eq!(drift_config.name, "__missing__");
        assert_eq!(drift_config.repository, "__missing__");
        assert_eq!(drift_config.version, "0.1.0");
        assert_eq!(
            drift_config.alert_config.dispatch_type,
            AlertDispatchType::Console
        );

        let new_alert_config = CustomMetricAlertConfig::new(
            Some(AlertDispatchType::Slack),
            Some("0 0 * * * *".to_string()),
            None,
        );

        // update
        drift_config
            .update_config_args(None, Some("test".to_string()), None, Some(new_alert_config))
            .unwrap();

        assert_eq!(drift_config.name, "test");
        assert_eq!(
            drift_config.alert_config.dispatch_type,
            AlertDispatchType::Slack
        );
        assert_eq!(
            drift_config.alert_config.schedule,
            "0 0 * * * *".to_string()
        );
    }

    #[test]
    fn test_custom_drift_profile() {
        let alert_config = CustomMetricAlertConfig::new(
            Some(AlertDispatchType::OpsGenie),
            Some("0 0 * * * *".to_string()),
            None,
        );
        let drift_config = CustomMetricDriftConfig::new(
            Some("scouter".to_string()),
            Some("ML".to_string()),
            Some("0.1.0".to_string()),
            Some(alert_config),
            None,
        )
        .unwrap();

        let custom_metrics = vec![
            CustomMetric::new("mae".to_string(), 12.4, AlertThreshold::Above, Some(2.3)).unwrap(),
            CustomMetric::new("accuracy".to_string(), 0.85, AlertThreshold::Below, None).unwrap(),
        ];

        let profile = CustomDriftProfile::new(drift_config, custom_metrics, None).unwrap();
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

    #[test]
    fn test_create_alert_description() {
        let alert_above_threshold = ComparisonMetricAlert {
            metric_name: "mse".to_string(),
            training_metric_value: 12.5,
            observed_metric_value: 14.0,
            alert_threshold_value: Some(1.0),
            alert_threshold: AlertThreshold::Above,
        };

        let description =
            alert_above_threshold.create_alert_description(AlertDispatchType::Console);
        assert!(description.contains(
            "The mse metric value has increased beyond the threshold (initial value + 1)"
        ));
        assert!(description.contains("Initial Metric Value: 12.5"));
        assert!(description.contains("Current Metric Value: 14"));

        let alert_below_threshold = ComparisonMetricAlert {
            metric_name: "accuracy".to_string(),
            training_metric_value: 0.9,
            observed_metric_value: 0.7,
            alert_threshold_value: None,
            alert_threshold: AlertThreshold::Below,
        };

        let description =
            alert_below_threshold.create_alert_description(AlertDispatchType::Console);
        assert!(
            description.contains("The accuracy metric value has dropped below the initial value")
        );
        assert!(description.contains("Initial Metric Value: 0.9"));
        assert!(description.contains("Current Metric Value: 0.7"));

        let alert_outside_threshold = ComparisonMetricAlert {
            metric_name: "mae".to_string(),
            training_metric_value: 12.5,
            observed_metric_value: 22.0,
            alert_threshold_value: Some(2.0),
            alert_threshold: AlertThreshold::Outside,
        };

        let description =
            alert_outside_threshold.create_alert_description(AlertDispatchType::Console);
        assert!(description
            .contains("The mae metric value has fallen outside the threshold (initial value ± 2)"));
        assert!(description.contains("Initial Metric Value: 12.5"));
        assert!(description.contains("Current Metric Value: 22"));
    }
}
