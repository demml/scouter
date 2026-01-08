use crate::error::TypeError;
use crate::AlertCondition;
use crate::{
    dispatch::AlertDispatchType, AlertDispatchConfig, AlertThreshold, CommonCrons,
    DispatchAlertDescription, OpsGenieDispatchConfig, SlackDispatchConfig, ValidateAlertConfig,
};
use core::fmt::Debug;
use pyo3::prelude::*;
use pyo3::types::PyString;
use serde::{Deserialize, Serialize};

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct GenAIAlertConfig {
    pub dispatch_config: AlertDispatchConfig,

    #[pyo3(get, set)]
    pub schedule: String,

    #[pyo3(get, set)]
    pub alert_condition: Option<AlertCondition>,
}

impl ValidateAlertConfig for GenAIAlertConfig {}

#[pymethods]
impl GenAIAlertConfig {
    #[new]
    #[pyo3(signature = (schedule=None, dispatch_config=None, alert_condition=None))]
    pub fn new(
        schedule: Option<&Bound<'_, PyAny>>,
        dispatch_config: Option<&Bound<'_, PyAny>>,
        alert_condition: Option<AlertCondition>,
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
            alert_condition,
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

impl Default for GenAIAlertConfig {
    fn default() -> GenAIAlertConfig {
        Self {
            dispatch_config: AlertDispatchConfig::default(),
            schedule: CommonCrons::EveryDay.cron(),
            alert_condition: None,
        }
    }
}

pub struct PromptComparisonMetricAlert {
    pub metric_name: String,
    pub training_metric_value: f64,
    pub observed_metric_value: f64,
    pub alert_threshold_value: Option<f64>,
    pub alert_threshold: AlertThreshold,
}

impl PromptComparisonMetricAlert {
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

impl DispatchAlertDescription for PromptComparisonMetricAlert {
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
#[cfg(feature = "mock")]
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
        let alert_condition = AlertCondition::new(5.0, AlertThreshold::Above, None);
        let alert_config = GenAIAlertConfig {
            dispatch_config,
            schedule,
            alert_condition: Some(alert_condition.clone()),
        };
        assert_eq!(alert_config.dispatch_type(), AlertDispatchType::OpsGenie);
    }
}
