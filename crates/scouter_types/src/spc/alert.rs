use crate::{
    CommonCrons, dispatch::AlertDispatchType, DispatchAlertDescription, ProfileFuncs,
    ValidateAlertConfig,
};
use core::fmt::Debug;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::Display;

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
    #[pyo3(signature = (rule=None, zones_to_monitor=None))]
    pub fn new(rule: Option<String>, zones_to_monitor: Option<Vec<AlertZone>>) -> Self {
        let rule = match rule {
            Some(r) => r,
            None => "8 16 4 8 2 4 1 1".to_string(),
        };

        let zones = zones_to_monitor.unwrap_or(
            [
                AlertZone::Zone1,
                AlertZone::Zone2,
                AlertZone::Zone3,
                AlertZone::Zone4,
            ]
            .to_vec(),
        );
        Self {
            rule,
            zones_to_monitor: zones,
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

    pub dispatch_type: AlertDispatchType,

    #[pyo3(get, set)]
    pub schedule: String,

    #[pyo3(get, set)]
    pub features_to_monitor: Vec<String>,

    #[pyo3(get, set)]
    pub dispatch_kwargs: HashMap<String, String>,
}

impl ValidateAlertConfig for SpcAlertConfig {}

#[pymethods]
impl SpcAlertConfig {
    #[new]
    #[pyo3(signature = (rule=None, dispatch_type=None, schedule=None, features_to_monitor=None, dispatch_kwargs=None))]
    pub fn new(
        rule: Option<SpcAlertRule>,
        dispatch_type: Option<AlertDispatchType>,
        schedule: Option<String>,
        features_to_monitor: Option<Vec<String>>,
        dispatch_kwargs: Option<HashMap<String, String>>,
    ) -> Self {
        let rule = rule.unwrap_or_default();

        let schedule = Self::resolve_schedule(schedule);
        let dispatch_type = dispatch_type.unwrap_or_default();
        let features_to_monitor = features_to_monitor.unwrap_or_default();
        let dispatch_kwargs = dispatch_kwargs.unwrap_or_default();

        Self {
            rule,
            dispatch_type,
            schedule,
            features_to_monitor,
            dispatch_kwargs,
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

impl Default for SpcAlertConfig {
    fn default() -> SpcAlertConfig {
        Self {
            rule: SpcAlertRule::default(),
            dispatch_type: AlertDispatchType::default(),
            schedule: CommonCrons::EveryDay.cron(),
            features_to_monitor: Vec::new(),
            dispatch_kwargs: HashMap::new(),
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
        let alert_config = SpcAlertConfig::new(None, None, None, None, None);
        assert_eq!(alert_config.dispatch_type, AlertDispatchType::Console);
        assert_eq!(alert_config.dispatch_type(), "Console");
        assert_eq!(AlertDispatchType::Console.value(), "Console");

        //test slack alert config
        let alert_config =
            SpcAlertConfig::new(None, Some(AlertDispatchType::Slack), None, None, None);
        assert_eq!(alert_config.dispatch_type, AlertDispatchType::Slack);
        assert_eq!(alert_config.dispatch_type(), "Slack");
        assert_eq!(AlertDispatchType::Slack.value(), "Slack");

        //test opsgenie alert config
        let mut alert_kwargs = HashMap::new();
        alert_kwargs.insert("channel".to_string(), "test".to_string());

        let alert_config = SpcAlertConfig::new(
            None,
            Some(AlertDispatchType::OpsGenie),
            None,
            None,
            Some(alert_kwargs),
        );
        assert_eq!(alert_config.dispatch_type, AlertDispatchType::OpsGenie);
        assert_eq!(alert_config.dispatch_type(), "OpsGenie");
        assert_eq!(alert_config.dispatch_kwargs.get("channel").unwrap(), "test");
        assert_eq!(AlertDispatchType::OpsGenie.value(), "OpsGenie");
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
