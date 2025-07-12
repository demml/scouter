use crate::error::TypeError;
use crate::{
    dispatch::AlertDispatchType, AlertDispatchConfig, AlertThreshold, CommonCrons,
    DispatchAlertDescription, OpsGenieDispatchConfig, ProfileFuncs, SlackDispatchConfig,
    ValidateAlertConfig,
};
use core::fmt::Debug;
use potato_head::prompt::ResponseType;
use potato_head::Prompt;
use pyo3::prelude::*;
use pyo3::types::PyString;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct LLMMetric {
    #[pyo3(get, set)]
    pub name: String,

    #[pyo3(get, set)]
    pub value: f64,

    #[pyo3(get)]
    pub prompt: Option<Prompt>,

    #[pyo3(get, set)]
    pub alert_condition: LLMMetricAlertCondition,
}

#[pymethods]
impl LLMMetric {
    #[new]
    #[pyo3(signature = (name, value, prompt, alert_threshold, alert_threshold_value=None))]
    pub fn new(
        name: &str,
        value: f64,
        prompt: Option<Prompt>,
        alert_threshold: AlertThreshold,
        alert_threshold_value: Option<f64>,
    ) -> Result<Self, TypeError> {
        // assert that the prompt is a scoring prompt
        if let Some(ref prompt) = prompt {
            if prompt.response_type != ResponseType::Score {
                return Err(TypeError::InvalidResponseType);
            }
        }

        let prompt_condition = LLMMetricAlertCondition::new(alert_threshold, alert_threshold_value);

        Ok(Self {
            name: name.to_lowercase(),
            value,
            prompt,
            alert_condition: prompt_condition,
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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct LLMMetricAlertCondition {
    #[pyo3(get, set)]
    pub alert_threshold: AlertThreshold,

    #[pyo3(get, set)]
    pub alert_threshold_value: Option<f64>,
}

#[pymethods]
#[allow(clippy::too_many_arguments)]
impl LLMMetricAlertCondition {
    #[new]
    #[pyo3(signature = (alert_threshold, alert_threshold_value=None))]
    pub fn new(alert_threshold: AlertThreshold, alert_threshold_value: Option<f64>) -> Self {
        Self {
            alert_threshold,
            alert_threshold_value,
        }
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct LLMMetricAlertConfig {
    pub dispatch_config: AlertDispatchConfig,

    #[pyo3(get, set)]
    pub schedule: String,

    #[pyo3(get, set)]
    pub alert_conditions: Option<HashMap<String, LLMMetricAlertCondition>>,
}

impl LLMMetricAlertConfig {
    pub fn set_alert_conditions(&mut self, metrics: &[LLMMetric]) {
        self.alert_conditions = Some(
            metrics
                .iter()
                .map(|m| (m.name.clone(), m.alert_condition.clone()))
                .collect(),
        );
    }
}

impl ValidateAlertConfig for LLMMetricAlertConfig {}

#[pymethods]
impl LLMMetricAlertConfig {
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

impl Default for LLMMetricAlertConfig {
    fn default() -> LLMMetricAlertConfig {
        Self {
            dispatch_config: AlertDispatchConfig::default(),
            schedule: CommonCrons::EveryDay.cron(),
            alert_conditions: None,
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
    use potato_head::create_score_prompt;

    #[test]
    fn test_alert_config() {
        //test console alert config
        let dispatch_config = AlertDispatchConfig::OpsGenie(OpsGenieDispatchConfig {
            team: "test-team".to_string(),
            priority: "P5".to_string(),
        });
        let schedule = "0 0 * * * *".to_string();
        let mut alert_config = LLMMetricAlertConfig {
            dispatch_config,
            schedule,
            ..Default::default()
        };
        assert_eq!(alert_config.dispatch_type(), AlertDispatchType::OpsGenie);

        let prompt = create_score_prompt();

        let llm_metrics = vec![
            LLMMetric::new(
                "mae",
                12.4,
                Some(prompt.clone()),
                AlertThreshold::Above,
                Some(2.3),
            )
            .unwrap(),
            LLMMetric::new(
                "accuracy",
                0.85,
                Some(prompt.clone()),
                AlertThreshold::Below,
                None,
            )
            .unwrap(),
        ];

        alert_config.set_alert_conditions(&llm_metrics);

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
}
