use crate::error::TypeError;
use crate::{
    dispatch::AlertDispatchType, AlertDispatchConfig, AlertThreshold, CommonCrons,
    DispatchAlertDescription, OpsGenieDispatchConfig, PyHelperFuncs, SlackDispatchConfig,
    ValidateAlertConfig,
};
use crate::{AlertCondition, AlertMap};
use core::fmt::Debug;
use pyo3::prelude::*;
use pyo3::types::PyString;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomMetric {
    #[pyo3(get, set)]
    pub name: String,

    #[pyo3(get, set)]
    pub baseline_value: f64,

    #[pyo3(get, set)]
    pub alert_condition: AlertCondition,
}

#[pymethods]
impl CustomMetric {
    #[new]
    #[pyo3(signature = (name, baseline_value, alert_threshold, delta=None))]
    pub fn new(
        name: &str,
        baseline_value: f64,
        alert_threshold: AlertThreshold,
        delta: Option<f64>,
    ) -> Result<Self, TypeError> {
        let custom_condition = AlertCondition::new(baseline_value, alert_threshold, delta);

        Ok(Self {
            name: name.to_lowercase(),
            baseline_value,
            alert_condition: custom_condition,
        })
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }

    #[getter]
    pub fn alert_threshold(&self) -> AlertThreshold {
        self.alert_condition.alert_threshold.clone()
    }

    #[getter]
    pub fn delta(&self) -> Option<f64> {
        self.alert_condition.delta
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct CustomMetricAlertConfig {
    pub dispatch_config: AlertDispatchConfig,

    #[pyo3(get, set)]
    pub schedule: String,

    #[pyo3(get, set)]
    pub alert_conditions: Option<HashMap<String, AlertCondition>>,
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
    #[pyo3(signature = (schedule=None, dispatch_config=None))]
    pub fn new(
        schedule: Option<&Bound<'_, PyAny>>,
        dispatch_config: Option<&Bound<'_, PyAny>>,
    ) -> Result<Self, TypeError> {
        let alert_dispatch_config = match dispatch_config {
            None => AlertDispatchConfig::default(),
            Some(config) => {
                if config.is_instance_of::<SlackDispatchConfig>() {
                    AlertDispatchConfig::Slack(config.extract::<SlackDispatchConfig>()?)
                } else if config.is_instance_of::<OpsGenieDispatchConfig>() {
                    AlertDispatchConfig::OpsGenie(config.extract::<OpsGenieDispatchConfig>()?)
                } else {
                    AlertDispatchConfig::default()
                }
            }
        };

        let schedule = match schedule {
            Some(schedule) => {
                if schedule.is_instance_of::<PyString>() {
                    schedule.to_string()
                } else if schedule.is_instance_of::<CommonCrons>() {
                    schedule.extract::<CommonCrons>().unwrap().cron()
                } else {
                    return Err(TypeError::InvalidScheduleError)?;
                }
            }
            None => CommonCrons::EveryDay.cron(),
        };

        let schedule = Self::resolve_schedule(&schedule);

        Ok(Self {
            schedule,
            dispatch_config: alert_dispatch_config,
            alert_conditions: None,
        })
    }

    #[getter]
    pub fn dispatch_type(&self) -> AlertDispatchType {
        self.dispatch_config.dispatch_type()
    }

    #[getter]
    pub fn dispatch_config<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.dispatch_config.config(py)
    }
}

impl Default for CustomMetricAlertConfig {
    fn default() -> CustomMetricAlertConfig {
        Self {
            dispatch_config: AlertDispatchConfig::default(),
            schedule: CommonCrons::EveryDay.cron(),
            alert_conditions: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct ComparisonMetricAlert {
    pub metric_name: String,
    pub baseline_value: f64,
    pub observed_value: f64,
    pub delta: Option<f64>,
    pub alert_threshold: AlertThreshold,
}

impl From<ComparisonMetricAlert> for AlertMap {
    fn from(val: ComparisonMetricAlert) -> Self {
        AlertMap::Custom(val)
    }
}

impl ComparisonMetricAlert {
    fn alert_description_header(&self) -> String {
        let below_threshold = |delta: Option<f64>| match delta {
            Some(b) => format!(
                "The observed {} metric value has dropped below the threshold (initial value - {})",
                self.metric_name, b
            ),
            None => format!(
                "The {} metric value has dropped below the initial value",
                self.metric_name
            ),
        };

        let above_threshold = |delta: Option<f64>| match delta {
            Some(b) => format!(
                "The {} metric value has increased beyond the threshold (initial value + {})",
                self.metric_name, b
            ),
            None => format!(
                "The {} metric value has increased beyond the initial value",
                self.metric_name
            ),
        };

        let outside_threshold = |delta: Option<f64>| match delta {
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
            AlertThreshold::Below => below_threshold(self.delta),
            AlertThreshold::Above => above_threshold(self.delta),
            AlertThreshold::Outside => outside_threshold(self.delta),
        }
    }
}

impl DispatchAlertDescription for ComparisonMetricAlert {
    // TODO make pretty per dispatch type
    fn create_alert_description(&self, _dispatch_type: AlertDispatchType) -> String {
        let mut alert_description = String::new();
        let header = format!("{}\n", self.alert_description_header());
        alert_description.push_str(&header);

        let current_metric = format!("Current Metric Value: {}\n", self.observed_value);
        let historical_metric = format!("Initial Metric Value: {}\n", self.baseline_value);

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
        let dispatch_config = AlertDispatchConfig::OpsGenie(OpsGenieDispatchConfig {
            team: "test-team".to_string(),
            priority: "P5".to_string(),
        });
        let schedule = "0 0 * * * *".to_string();
        let mut alert_config = CustomMetricAlertConfig {
            dispatch_config,
            schedule,
            ..Default::default()
        };
        assert_eq!(alert_config.dispatch_type(), AlertDispatchType::OpsGenie);

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
            assert_eq!(alert_conditions["mae"].delta, Some(2.3));
            assert_eq!(
                alert_conditions["accuracy"].alert_threshold,
                AlertThreshold::Below
            );
            assert_eq!(alert_conditions["accuracy"].delta, None);
        } else {
            panic!("alert_conditions should not be None");
        }
    }

    #[test]
    fn test_create_alert_description() {
        let alert_above_threshold = ComparisonMetricAlert {
            metric_name: "mse".to_string(),
            baseline_value: 12.5,
            observed_value: 14.0,
            delta: Some(1.0),
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
            baseline_value: 0.9,
            observed_value: 0.7,
            delta: None,
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
            baseline_value: 12.5,
            observed_value: 22.0,
            delta: Some(2.0),
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
