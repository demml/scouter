use crate::{
    dispatch::AlertDispatchType, AlertDispatchConfig, CommonCrons, DispatchAlertDescription,
    OpsGenieDispatchConfig, ProfileFuncs, SlackDispatchConfig, ValidateAlertConfig,
};
use core::fmt::Debug;
use pyo3::prelude::*;
use pyo3::types::PyString;
use scouter_error::PyScouterError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Display;
use tracing::error;

#[pyclass(eq)]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, std::cmp::Eq, Hash)]
pub enum AlertZone {
    Zone1,
    Zone2,
    Zone3,
    Zone4,
    NotApplicable,
}

impl Display for AlertZone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertZone::Zone1 => write!(f, "Zone 1"),
            AlertZone::Zone2 => write!(f, "Zone 2"),
            AlertZone::Zone3 => write!(f, "Zone 3"),
            AlertZone::Zone4 => write!(f, "Zone 4"),
            AlertZone::NotApplicable => write!(f, "NA"),
        }
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct SpcAlertRule {
    #[pyo3(get, set)]
    pub rule: String,

    #[pyo3(get, set)]
    pub zones_to_monitor: Vec<AlertZone>,
}

#[pymethods]
impl SpcAlertRule {
    #[new]
    #[pyo3(signature = (rule="8 16 4 8 2 4 1 1", zones_to_monitor=vec![
        AlertZone::Zone1,
        AlertZone::Zone2,
        AlertZone::Zone3,
        AlertZone::Zone4,
    ]))]
    pub fn new(rule: &str, zones_to_monitor: Vec<AlertZone>) -> Self {
        Self {
            rule: rule.to_string(),
            zones_to_monitor,
        }
    }
}

impl Default for SpcAlertRule {
    fn default() -> SpcAlertRule {
        Self {
            rule: "8 16 4 8 2 4 1 1".to_string(),
            zones_to_monitor: vec![
                AlertZone::Zone1,
                AlertZone::Zone2,
                AlertZone::Zone3,
                AlertZone::Zone4,
            ],
        }
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct SpcAlertConfig {
    #[pyo3(get, set)]
    pub rule: SpcAlertRule,

    #[pyo3(get, set)]
    pub schedule: String,

    #[pyo3(get, set)]
    pub features_to_monitor: Vec<String>,

    pub dispatch_config: AlertDispatchConfig,
}

impl ValidateAlertConfig for SpcAlertConfig {}

#[pymethods]
impl SpcAlertConfig {
    #[new]
    #[pyo3(signature = (rule=SpcAlertRule::default(), schedule=None, features_to_monitor=vec![], dispatch_config=None))]
    pub fn new(
        rule: SpcAlertRule,
        schedule: Option<&Bound<'_, PyAny>>,
        features_to_monitor: Vec<String>,
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

        // check if schedule is None, string or CommonCrons

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
            rule,
            schedule,
            features_to_monitor,
            dispatch_config: alert_dispatch_config,
        })
    }

    #[getter]
    pub fn dispatch_type(&self) -> String {
        match self.dispatch_config {
            AlertDispatchConfig::Slack(_) => "Slack".to_string(),
            AlertDispatchConfig::Console => "Console".to_string(),
            AlertDispatchConfig::OpsGenie(_) => "OpsGenie".to_string(),
        }
    }
}

impl Default for SpcAlertConfig {
    fn default() -> SpcAlertConfig {
        Self {
            rule: SpcAlertRule::default(),
            dispatch_config: AlertDispatchConfig::default(),
            schedule: CommonCrons::EveryDay.cron(),
            features_to_monitor: Vec::new(),
        }
    }
}

#[pyclass(eq)]
#[derive(Debug, Eq, Hash, PartialEq, Serialize, Deserialize, Clone, Copy)]
pub enum SpcAlertType {
    OutOfBounds,
    Consecutive,
    Alternating,
    AllGood,
    Trend,
}

impl Display for SpcAlertType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpcAlertType::OutOfBounds => write!(f, "Out of bounds"),
            SpcAlertType::Consecutive => write!(f, "Consecutive"),
            SpcAlertType::Alternating => write!(f, "Alternating"),
            SpcAlertType::AllGood => write!(f, "All good"),
            SpcAlertType::Trend => write!(f, "Trend"),
        }
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, Hash, PartialEq)]
pub struct SpcAlert {
    #[pyo3(get)]
    pub kind: SpcAlertType,

    #[pyo3(get)]
    pub zone: AlertZone,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl SpcAlert {
    #[new]
    pub fn new(kind: SpcAlertType, zone: AlertZone) -> Self {
        Self { kind, zone }
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}

// Drift config to use when calculating drift on a new sample of data

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpcFeatureAlert {
    #[pyo3(get)]
    pub feature: String,

    #[pyo3(get)]
    pub alerts: Vec<SpcAlert>,
}

impl SpcFeatureAlert {
    pub fn new(feature: String, alerts: Vec<SpcAlert>) -> Self {
        Self { feature, alerts }
    }
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl SpcFeatureAlert {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpcFeatureAlerts {
    #[pyo3(get)]
    pub features: HashMap<String, SpcFeatureAlert>,

    #[pyo3(get)]
    pub has_alerts: bool,
}

impl SpcFeatureAlerts {
    // rust-only function to insert feature alerts
    pub fn insert_feature_alert(&mut self, feature: &str, alerts: HashSet<SpcAlert>) {
        // convert the alerts to a vector
        let alerts: Vec<SpcAlert> = alerts.into_iter().collect();

        let feature_alert = SpcFeatureAlert::new(feature.to_string(), alerts);

        self.features.insert(feature.to_string(), feature_alert);
    }
}

impl DispatchAlertDescription for SpcFeatureAlerts {
    fn create_alert_description(&self, dispatch_type: AlertDispatchType) -> String {
        let mut alert_description = String::new();

        for (i, (_, feature_alert)) in self.features.iter().enumerate() {
            if feature_alert.alerts.is_empty() {
                continue;
            }
            if i == 0 {
                let header = match dispatch_type {
                    AlertDispatchType::Console => "Features that have drifted: \n",
                    AlertDispatchType::OpsGenie => {
                        "Drift has been detected for the following features:\n"
                    }
                    AlertDispatchType::Slack => {
                        "Drift has been detected for the following features:\n"
                    }
                };
                alert_description.push_str(header);
            }

            let feature_name = match dispatch_type {
                AlertDispatchType::Console | AlertDispatchType::OpsGenie => {
                    format!("{:indent$}{}: \n", "", &feature_alert.feature, indent = 4)
                }
                AlertDispatchType::Slack => format!("{}: \n", &feature_alert.feature),
            };

            alert_description = format!("{}{}", alert_description, feature_name);
            feature_alert.alerts.iter().for_each(|alert| {
                let alert_details = match dispatch_type {
                    AlertDispatchType::Console | AlertDispatchType::OpsGenie => {
                        let kind = format!("{:indent$}Kind: {}\n", "", &alert.kind, indent = 8);
                        let zone = format!("{:indent$}Zone: {}\n", "", &alert.zone, indent = 8);
                        format!("{}{}", kind, zone)
                    }
                    AlertDispatchType::Slack => format!(
                        "{:indent$}{} error in {}\n",
                        "",
                        &alert.kind,
                        &alert.zone,
                        indent = 4
                    ),
                };
                alert_description = format!("{}{}", alert_description, alert_details);
            });
        }
        alert_description
    }
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl SpcFeatureAlerts {
    #[new]
    pub fn new(has_alerts: bool) -> Self {
        Self {
            features: HashMap::new(),
            has_alerts,
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
}

pub struct TaskAlerts {
    pub alerts: SpcFeatureAlerts,
}

impl TaskAlerts {
    pub fn new() -> Self {
        Self {
            alerts: SpcFeatureAlerts::new(false),
        }
    }
}

impl Default for TaskAlerts {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_types() {
        // write tests for all alerts
        let control_alert = SpcAlertRule::default().rule;

        assert_eq!(control_alert, "8 16 4 8 2 4 1 1");
        assert_eq!(AlertZone::NotApplicable.to_string(), "NA");
        assert_eq!(AlertZone::Zone1.to_string(), "Zone 1");
        assert_eq!(AlertZone::Zone2.to_string(), "Zone 2");
        assert_eq!(AlertZone::Zone3.to_string(), "Zone 3");
        assert_eq!(AlertZone::Zone4.to_string(), "Zone 4");
        assert_eq!(SpcAlertType::AllGood.to_string(), "All good");
        assert_eq!(SpcAlertType::Consecutive.to_string(), "Consecutive");
        assert_eq!(SpcAlertType::Alternating.to_string(), "Alternating");
        assert_eq!(SpcAlertType::OutOfBounds.to_string(), "Out of bounds");
    }

    #[test]
    fn test_alert_config() {
        //test console alert config
        let alert_config = SpcAlertConfig::default();
        assert_eq!(alert_config.dispatch_config, AlertDispatchConfig::Console);
        assert_eq!(alert_config.dispatch_type(), "Console");

        let slack_dispatch_config = SlackDispatchConfig {
            channel: "test-channel".to_string(),
        };
        //test slack alert config
        let alert_config = SpcAlertConfig {
            dispatch_config: AlertDispatchConfig::Slack(slack_dispatch_config.clone()),
            ..Default::default()
        };
        assert_eq!(
            alert_config.dispatch_config,
            AlertDispatchConfig::Slack(slack_dispatch_config)
        );
        assert_eq!(alert_config.dispatch_type(), "Slack");
        assert_eq!(
            match &alert_config.dispatch_config {
                AlertDispatchConfig::Slack(config) => &config.channel,
                _ => panic!("Expected Slack dispatch config"),
            },
            "test-channel"
        );

        //test opsgenie alert config
        let opsgenie_dispatch_config = OpsGenieDispatchConfig {
            team: "test-team".to_string(),
        };

        let alert_config = SpcAlertConfig {
            dispatch_config: AlertDispatchConfig::OpsGenie(opsgenie_dispatch_config.clone()),
            ..Default::default()
        };
        assert_eq!(
            alert_config.dispatch_config,
            AlertDispatchConfig::OpsGenie(opsgenie_dispatch_config.clone())
        );
        assert_eq!(alert_config.dispatch_type(), "OpsGenie");
        assert_eq!(
            match &alert_config.dispatch_config {
                AlertDispatchConfig::OpsGenie(config) => &config.team,
                _ => panic!("Expected OpsGenie dispatch config"),
            },
            "test-team"
        );
    }

    #[test]
    fn test_spc_feature_alerts() {
        // Create a sample SpcFeatureAlert (assuming SpcFeatureAlert is defined elsewhere)
        let sample_alert = SpcFeatureAlert {
            feature: "feature1".to_string(),
            alerts: vec![SpcAlert {
                kind: SpcAlertType::OutOfBounds,
                zone: AlertZone::Zone1,
            }]
            .into_iter()
            .collect(),
            // Initialize fields of SpcFeatureAlert
        };

        // Create a HashMap with sample data
        let mut features = HashMap::new();
        features.insert("feature1".to_string(), sample_alert.clone());

        // Create an instance of SpcFeatureAlerts
        let alerts = SpcFeatureAlerts {
            features: features.clone(),
            has_alerts: true,
        };

        // Assert the values
        assert!(alerts.has_alerts);

        // Assert the values of the features
        assert_eq!(alerts.features["feature1"].feature, sample_alert.feature);

        // Assert constructing alert description
        let _ = alerts.create_alert_description(AlertDispatchType::Console);
        let _ = alerts.create_alert_description(AlertDispatchType::OpsGenie);
        let _ = alerts.create_alert_description(AlertDispatchType::Slack);
    }
}
