use crate::{
    AlertDispatchConfig, AlertDispatchType, CommonCrons, DispatchAlertDescription,
    OpsGenieDispatchConfig, SlackDispatchConfig, ValidateAlertConfig,
};
use core::fmt::Debug;
use pyo3::prelude::*;
use pyo3::types::PyString;
use scouter_error::PyScouterError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::error;

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PsiAlertConfig {
    #[pyo3(get, set)]
    pub schedule: String,

    #[pyo3(get, set)]
    pub features_to_monitor: Vec<String>,

    #[pyo3(get, set)]
    pub psi_threshold: f64,

    pub dispatch_config: AlertDispatchConfig,
}

impl Default for PsiAlertConfig {
    fn default() -> PsiAlertConfig {
        Self {
            schedule: CommonCrons::EveryDay.cron(),
            features_to_monitor: Vec::new(),
            psi_threshold: 0.25,
            dispatch_config: AlertDispatchConfig::default(),
        }
    }
}

impl ValidateAlertConfig for PsiAlertConfig {}

#[pymethods]
impl PsiAlertConfig {
    #[new]
    #[pyo3(signature = (schedule=None, features_to_monitor=vec![], psi_threshold=0.25, dispatch_config=None))]
    pub fn new(
        schedule: Option<&Bound<'_, PyAny>>,
        features_to_monitor: Vec<String>,
        psi_threshold: f64,
        dispatch_config: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
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
                    error!("Invalid schedule type");
                    return Err(PyScouterError::new_err("Invalid schedule type"))?;
                }
            }
            None => CommonCrons::EveryDay.cron(),
        };

        let schedule = Self::resolve_schedule(&schedule);

        Ok(Self {
            schedule,
            features_to_monitor,
            psi_threshold,
            dispatch_config: alert_dispatch_config,
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

pub struct PsiFeatureAlerts {
    pub features: HashMap<String, f64>,
    pub threshold: f64,
}

impl DispatchAlertDescription for PsiFeatureAlerts {
    fn create_alert_description(&self, dispatch_type: AlertDispatchType) -> String {
        let mut alert_description = String::new();

        for (i, (feature_name, drift_value)) in self.features.iter().enumerate() {
            let description = format!("Feature '{}' has experienced drift, with a current PSI score of {} that exceeds the configured threshold of {}.", feature_name, drift_value, self.threshold);

            if i == 0 {
                let header = "PSI Drift has been detected for the following features:\n";
                alert_description.push_str(header);
            }

            let feature_name = match dispatch_type {
                AlertDispatchType::Console | AlertDispatchType::OpsGenie => {
                    format!("{:indent$}{}: \n", "", &feature_name, indent = 4)
                }
                AlertDispatchType::Slack => format!("{}: \n", &feature_name),
            };

            alert_description = format!("{}{}", alert_description, feature_name);

            let alert_details = match dispatch_type {
                AlertDispatchType::Console | AlertDispatchType::OpsGenie => {
                    format!("{:indent$}Drift Value: {}\n", "", description, indent = 8)
                }
                AlertDispatchType::Slack => {
                    format!("{:indent$}Drift Value: {}\n", "", description, indent = 4)
                }
            };
            alert_description = format!("{}{}", alert_description, alert_details);
        }
        alert_description
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_config() {
        //test console alert config
        let alert_config = PsiAlertConfig::default();
        assert_eq!(alert_config.dispatch_config, AlertDispatchConfig::default());
        assert_eq!(alert_config.dispatch_type(), AlertDispatchType::Console);

        //test slack alert config
        let slack_alert_dispatch_config = SlackDispatchConfig {
            channel: "test".to_string(),
        };
        let alert_config = PsiAlertConfig {
            dispatch_config: AlertDispatchConfig::Slack(slack_alert_dispatch_config.clone()),
            ..Default::default()
        };
        assert_eq!(
            alert_config.dispatch_config,
            AlertDispatchConfig::Slack(slack_alert_dispatch_config)
        );
        assert_eq!(alert_config.dispatch_type(), AlertDispatchType::Slack);

        //test opsgenie alert config
        let opsgenie_dispatch_config = AlertDispatchConfig::OpsGenie(OpsGenieDispatchConfig {
            team: "test-team".to_string(),
        });
        let alert_config = PsiAlertConfig {
            dispatch_config: opsgenie_dispatch_config.clone(),
            ..Default::default()
        };

        assert_eq!(
            alert_config.dispatch_config,
            opsgenie_dispatch_config.clone()
        );
        assert_eq!(alert_config.dispatch_type(), AlertDispatchType::OpsGenie);
        assert_eq!(
            match &alert_config.dispatch_config {
                AlertDispatchConfig::OpsGenie(config) => &config.team,
                _ => panic!("Expected OpsGenie dispatch config"),
            },
            "test-team"
        );
    }

    #[test]
    fn test_create_alert_description() {
        let features = HashMap::from([
            ("feature1".to_string(), 0.35),
            ("feature2".to_string(), 0.45),
        ]);
        let threshold = 0.3;
        let psi_feature_alerts = PsiFeatureAlerts {
            features,
            threshold,
        };

        // Test for Console dispatch type
        let description = psi_feature_alerts.create_alert_description(AlertDispatchType::Console);
        assert!(description.contains("PSI Drift has been detected for the following features:"));
        assert!(description.contains("Feature 'feature1' has experienced drift, with a current PSI score of 0.35 that exceeds the configured threshold of 0.3."));
        assert!(description.contains("Feature 'feature2' has experienced drift, with a current PSI score of 0.45 that exceeds the configured threshold of 0.3."));

        // Test for Slack dispatch type
        let description = psi_feature_alerts.create_alert_description(AlertDispatchType::Slack);
        assert!(description.contains("PSI Drift has been detected for the following features:"));
        assert!(description.contains("Feature 'feature1' has experienced drift, with a current PSI score of 0.35 that exceeds the configured threshold of 0.3."));
        assert!(description.contains("Feature 'feature2' has experienced drift, with a current PSI score of 0.45 that exceeds the configured threshold of 0.3."));

        // Test for OpsGenie dispatch type
        let description = psi_feature_alerts.create_alert_description(AlertDispatchType::OpsGenie);
        assert!(description.contains("PSI Drift has been detected for the following features:"));
        assert!(description.contains("Feature 'feature1' has experienced drift, with a current PSI score of 0.35 that exceeds the configured threshold of 0.3."));
        assert!(description.contains("Feature 'feature2' has experienced drift, with a current PSI score of 0.45 that exceeds the configured threshold of 0.3."));
    }
}
