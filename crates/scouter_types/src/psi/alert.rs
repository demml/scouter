use crate::error::TypeError;
use crate::{
    AlertDispatchConfig, AlertDispatchType, CommonCrons, DispatchAlertDescription,
    OpsGenieDispatchConfig, SlackDispatchConfig, ValidateAlertConfig,
};
use core::fmt::Debug;
use pyo3::prelude::*;
use pyo3::types::PyString;
use pyo3::IntoPyObjectExt;
use serde::{Deserialize, Serialize};
use statrs::distribution::{ChiSquared, ContinuousCDF, Normal};

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum PsiThresholdConfig {
    Normal(PsiNormalThreshold),
    ChiSquare(PsiChiSquareThreshold),
    Fixed(PsiFixedThreshold),
}

impl PsiThresholdConfig {
    pub fn config<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match self {
            PsiThresholdConfig::Normal(config) => config.clone().into_bound_py_any(py),
            PsiThresholdConfig::ChiSquare(config) => config.clone().into_bound_py_any(py),
            PsiThresholdConfig::Fixed(config) => config.clone().into_bound_py_any(py),
        }
    }

    pub fn compute_threshold(&self, target_sample_size: u64, bin_count: u64) -> f64 {
        match self {
            PsiThresholdConfig::Normal(normal) => {
                normal.compute_threshold(target_sample_size, bin_count)
            }
            PsiThresholdConfig::ChiSquare(chi) => {
                chi.compute_threshold(target_sample_size, bin_count)
            }
            PsiThresholdConfig::Fixed(fixed) => fixed.compute_threshold(),
        }
    }
}

impl Default for PsiThresholdConfig {
    fn default() -> Self {
        PsiThresholdConfig::ChiSquare(PsiChiSquareThreshold { alpha: 0.05 })
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PsiNormalThreshold {
    #[pyo3(get, set)]
    pub alpha: f64,
}

impl PsiNormalThreshold {
    /// Based on Yurdakul (2018) "Statistical Properties of Population Stability Index"
    /// Method I (Section 3.1.1): Normal approximation for one-sample case (fixed base)
    ///
    /// Paper: https://scholarworks.wmich.edu/dissertations/3208
    ///
    /// Formula: PSI > (B-1)/M + z_α × √[2(B-1)]/M
    /// where the base population is treated as fixed and only target sample is random
    #[allow(non_snake_case)]
    pub fn compute_threshold(&self, target_sample_size: u64, bin_count: u64) -> f64 {
        let M = target_sample_size as f64;
        let B = bin_count as f64;

        let normal = Normal::new(0.0, 1.0).unwrap();
        let z_alpha = normal.inverse_cdf(1.0 - self.alpha);

        let exp_val = (B - 1.0) / M;
        let std_dev = (2.0 * (B - 1.0)).sqrt() / M;

        exp_val + z_alpha * std_dev
    }
}

#[pymethods]
impl PsiNormalThreshold {
    #[new]
    #[pyo3(signature = (alpha=0.05))]
    pub fn new(alpha: f64) -> PyResult<Self> {
        if !(0.0..1.0).contains(&alpha) {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "alpha must be between 0.0 and 1.0 (exclusive)",
            ));
        }
        Ok(Self { alpha })
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PsiChiSquareThreshold {
    #[pyo3(get, set)]
    pub alpha: f64,
}

impl PsiChiSquareThreshold {
    /// Based on Yurdakul (2018) "Statistical Properties of Population Stability Index"
    /// Method II (Section 3.1.2): Chi-square approximation for one-sample case (fixed base)
    ///
    /// Paper: https://scholarworks.wmich.edu/dissertations/3208
    ///
    /// Formula: PSI > χ²_{α,B-1} × (1/M)
    /// where the base population is treated as fixed and only target sample is random
    #[allow(non_snake_case)]
    pub fn compute_threshold(&self, target_sample_size: u64, bin_count: u64) -> f64 {
        let M = target_sample_size as f64;
        let B = bin_count as f64;
        let chi2 = ChiSquared::new(B - 1.0).unwrap();
        let chi2_critical = chi2.inverse_cdf(1.0 - self.alpha);

        chi2_critical / M
    }
}

#[pymethods]
impl PsiChiSquareThreshold {
    #[new]
    #[pyo3(signature = (alpha=0.05))]
    pub fn new(alpha: f64) -> PyResult<Self> {
        if !(0.0..1.0).contains(&alpha) {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "alpha must be between 0.0 and 1.0 (exclusive)",
            ));
        }
        Ok(Self { alpha })
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PsiFixedThreshold {
    #[pyo3(get, set)]
    pub threshold: f64,
}

impl PsiFixedThreshold {
    pub fn compute_threshold(&self) -> f64 {
        self.threshold
    }
}

#[pymethods]
impl PsiFixedThreshold {
    #[new]
    #[pyo3(signature = (threshold=0.25))]
    pub fn new(threshold: f64) -> PyResult<Self> {
        if threshold < 0.0 {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                "Threshold values must be non-zero",
            ));
        }
        Ok(Self { threshold })
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PsiAlertConfig {
    #[pyo3(get, set)]
    pub schedule: String,

    #[pyo3(get, set)]
    pub features_to_monitor: Vec<String>,

    pub dispatch_config: AlertDispatchConfig,

    pub threshold_config: Option<PsiThresholdConfig>,
}

impl Default for PsiAlertConfig {
    fn default() -> PsiAlertConfig {
        Self {
            schedule: CommonCrons::EveryDay.cron(),
            features_to_monitor: Vec::new(),
            dispatch_config: AlertDispatchConfig::default(),
            threshold_config: None,
        }
    }
}

impl ValidateAlertConfig for PsiAlertConfig {}

#[pymethods]
impl PsiAlertConfig {
    #[new]
    #[pyo3(signature = (schedule=None, features_to_monitor=vec![], dispatch_config=None, threshold_config=None))]
    pub fn new(
        schedule: Option<&Bound<'_, PyAny>>,
        features_to_monitor: Vec<String>,
        dispatch_config: Option<&Bound<'_, PyAny>>,
        threshold_config: Option<&Bound<'_, PyAny>>,
    ) -> Result<Self, TypeError> {
        let dispatch_config = match dispatch_config {
            None => AlertDispatchConfig::default(),
            Some(config) => {
                if config.is_instance_of::<SlackDispatchConfig>() {
                    AlertDispatchConfig::Slack(config.extract()?)
                } else if config.is_instance_of::<OpsGenieDispatchConfig>() {
                    AlertDispatchConfig::OpsGenie(config.extract()?)
                } else {
                    return Err(TypeError::InvalidDispatchConfigError);
                }
            }
        };

        let threshold_config = match threshold_config {
            None => None,
            Some(config) => {
                if config.is_instance_of::<PsiNormalThreshold>() {
                    Some(PsiThresholdConfig::Normal(config.extract()?))
                } else if config.is_instance_of::<PsiChiSquareThreshold>() {
                    Some(PsiThresholdConfig::ChiSquare(config.extract()?))
                } else if config.is_instance_of::<PsiFixedThreshold>() {
                    // ← Fixed bug
                    Some(PsiThresholdConfig::Fixed(config.extract()?))
                } else {
                    return Err(TypeError::InvalidPsiThresholdConfigError);
                }
            }
        };

        let schedule = match schedule {
            Some(schedule) => {
                if schedule.is_instance_of::<PyString>() {
                    schedule.to_string()
                } else if schedule.is_instance_of::<CommonCrons>() {
                    schedule.extract::<CommonCrons>()?.cron()
                } else {
                    return Err(TypeError::InvalidScheduleError);
                }
            }
            None => CommonCrons::EveryDay.cron(),
        };

        let schedule = Self::resolve_schedule(&schedule);

        Ok(Self {
            schedule,
            features_to_monitor,
            dispatch_config,
            threshold_config,
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

    #[getter]
    pub fn threshold_config<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match &self.threshold_config {
            None => Ok(py.None().into_bound_py_any(py)?),
            Some(config) => config.config(py),
        }
    }
}

#[derive(Clone)]
pub struct PsiFeatureAlert {
    pub feature: String,
    pub drift: f64,
    pub threshold: f64,
}

pub struct PsiFeatureAlerts {
    pub alerts: Vec<PsiFeatureAlert>,
}

impl DispatchAlertDescription for PsiFeatureAlerts {
    fn create_alert_description(&self, dispatch_type: AlertDispatchType) -> String {
        let mut alert_description = String::new();

        for (i, alert) in self.alerts.iter().enumerate() {
            let description = format!("Feature '{}' has experienced drift, with a current PSI score of {} that exceeds the configured threshold of {}.", alert.feature, alert.drift, alert.threshold);

            if i == 0 {
                let header = "PSI Drift has been detected for the following features:\n";
                alert_description.push_str(header);
            }

            let feature_name = match dispatch_type {
                AlertDispatchType::Console | AlertDispatchType::OpsGenie => {
                    format!("{:indent$}{}: \n", "", alert.feature, indent = 4)
                }
                AlertDispatchType::Slack => format!("{}: \n", alert.feature),
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
            priority: "P5".to_string(),
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
