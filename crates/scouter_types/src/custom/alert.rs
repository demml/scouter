use crate::{
    dispatch::AlertDispatchType, CommonCrons, DispatchAlertDescription, ProfileFuncs,
    ValidateAlertConfig,
};
use core::fmt::Debug;
use pyo3::prelude::*;
use scouter_error::{CustomMetricError, ScouterError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;
use std::fmt::Formatter;
use pyo3::types::PyString;
use tracing::error;
use scouter_error::PyScouterError;

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
        name: &str,
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

    #[getter]
    pub fn class_id(&self) -> String {
        self.name.clone()
    }
}

#[pyclass(eq)]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum AlertThreshold {
    Below,
    Above,
    Outside,
}

impl Display for AlertThreshold {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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
    pub fn set_alert_conditions(&mut self, metrics: &[CustomMetric]) {
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
    #[pyo3(signature = (dispatch_type=AlertDispatchType::default(), schedule=None, dispatch_kwargs=HashMap::new()))]
    pub fn new(
        dispatch_type: AlertDispatchType,
        schedule: Option<&Bound<'_, PyAny>>,
        dispatch_kwargs: HashMap<String, String>,
    ) -> PyResult<Self> {

        let schedule = match schedule {
            Some(schedule) => {
                if schedule.is_instance_of::<PyString>() {
                    schedule.to_string()
                } else if schedule.is_instance_of::<CommonCrons>() {
                    schedule.extract::<CommonCrons>().unwrap().cron()
                } else {
                    error!("Invalid schedule type");
                    return Err(PyScouterError::new_err("Invalid schedule type"))?;
                }
            }
            None => CommonCrons::EveryDay.cron(),
        };

        let schedule = Self::resolve_schedule(&schedule);

        Ok(Self {
            dispatch_type,
            schedule,
            dispatch_kwargs,
            alert_conditions: None,
        })
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
            schedule: CommonCrons::EveryDay.cron(),
            dispatch_kwargs: HashMap::new(),
            alert_conditions: None,
        }
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
        let mut alert_config = CustomMetricAlertConfig::default();

        alert_config.dispatch_type = dispatch_type;
        alert_config.schedule = schedule;

        assert_eq!(alert_config.dispatch_type(), "OpsGenie");

        let custom_metrics = vec![
            CustomMetric::new("mae", 12.4, AlertThreshold::Above, Some(2.3)).unwrap(),
            CustomMetric::new("accuracy", 0.85, AlertThreshold::Below, None).unwrap(),
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
