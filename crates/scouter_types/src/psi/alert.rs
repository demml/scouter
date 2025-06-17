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
pub enum PsiThreshold {
    Normal(PsiNormalThreshold),
    ChiSquare(PsiChiSquareThreshold),
    Fixed(PsiFixedThreshold),
}

impl PsiThreshold {
    pub fn config<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match self {
            PsiThreshold::Normal(config) => config.clone().into_bound_py_any(py),
            PsiThreshold::ChiSquare(config) => config.clone().into_bound_py_any(py),
            PsiThreshold::Fixed(config) => config.clone().into_bound_py_any(py),
        }
    }

    pub fn compute_threshold(&self, target_sample_size: u64, bin_count: u64) -> f64 {
        match self {
            PsiThreshold::Normal(normal) => normal.compute_threshold(target_sample_size, bin_count),
            PsiThreshold::ChiSquare(chi) => chi.compute_threshold(target_sample_size, bin_count),
            PsiThreshold::Fixed(fixed) => fixed.compute_threshold(),
        }
    }
}

impl Default for PsiThreshold {
    // Default threshold is ChiSquare with alpha = 0.05
    fn default() -> Self {
        PsiThreshold::ChiSquare(PsiChiSquareThreshold { alpha: 0.05 })
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

    pub threshold: PsiThreshold,
}

impl Default for PsiAlertConfig {
    fn default() -> PsiAlertConfig {
        Self {
            schedule: CommonCrons::EveryDay.cron(),
            features_to_monitor: Vec::new(),
            dispatch_config: AlertDispatchConfig::default(),
            threshold: PsiThreshold::default(),
        }
    }
}

impl ValidateAlertConfig for PsiAlertConfig {}

#[pymethods]
impl PsiAlertConfig {
    #[new]
    #[pyo3(signature = (schedule=None, features_to_monitor=vec![], dispatch_config=None, threshold=None))]
    pub fn new(
        schedule: Option<&Bound<'_, PyAny>>,
        features_to_monitor: Vec<String>,
        dispatch_config: Option<&Bound<'_, PyAny>>,
        threshold: Option<&Bound<'_, PyAny>>,
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

        let threshold = match threshold {
            None => PsiThreshold::default(),
            Some(config) => {
                if config.is_instance_of::<PsiNormalThreshold>() {
                    PsiThreshold::Normal(config.extract()?)
                } else if config.is_instance_of::<PsiChiSquareThreshold>() {
                    PsiThreshold::ChiSquare(config.extract()?)
                } else if config.is_instance_of::<PsiFixedThreshold>() {
                    // ← Fixed bug
                    PsiThreshold::Fixed(config.extract()?)
                } else {
                    return Err(TypeError::InvalidPsiThresholdError);
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
            threshold,
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
    pub fn threshold<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.threshold.config(py)
    }
}

#[derive(Clone, Debug)]
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
    use approx::assert_relative_eq;

    #[test]
    fn test_compute_threshold_method_i_paper_validation() {
        // Test based on Yurdakul (2018) Method I: Normal approximation for fixed base population
        //
        // Test case from Table 3.1 in the paper
        // B = 10 bins, M = 400 sample size, α = 0.05 (95th percentile)
        let threshold = PsiNormalThreshold { alpha: 0.05 };
        let result = threshold.compute_threshold(400, 10);

        // From Table 3.1: Expected ~4.0% for N=∞, M=400, B=10 using normal approximation
        // Expected value = 9 / 400 = 0.0225
        // Std dev = sqrt(2 * 9) / 400 ≈ 4.24 / 400 ≈ 0.0106
        // z_α for 95th percentile ≈ 1.645
        // Threshold ≈ 0.0225 + 1.645 * 0.0106 ≈ 0.0400
        assert_relative_eq!(result, 0.0400, epsilon = 0.002);
    }

    #[test]
    fn test_compute_threshold_method_ii_paper_validation() {
        // Test based on Yurdakul (2018) Method II: PSI > χ²_{α,B-1} × (1/M)

        // Test case from Tables 3.2 and 3.4 in the paper
        // B=10 bins, M=400 sample size, α=0.05 (95th percentile)
        let threshold = PsiChiSquareThreshold { alpha: 0.05 };
        let result = threshold.compute_threshold(400, 10);

        // From Table 3.2: Expected ~8.5% for N=∞, M=400, B=10
        // Chi-square with 9 df at 95th percentile ≈ 16.919
        // Expected: 16.919 / 400 ≈ 0.0423 (4.23%)
        assert_relative_eq!(result, 0.0423, epsilon = 0.002);

        // Test case: B=20 bins, M=1000 sample size, α=0.05
        let result_20_bins = threshold.compute_threshold(1000, 20);
        // Chi-square with 19 df at 95th percentile ≈ 30.144
        // Expected: 30.144 / 1000 ≈ 0.0301 (3.01%)
        assert_relative_eq!(result_20_bins, 0.0301, epsilon = 0.002);
    }

    #[test]
    fn test_compute_threshold_paper_table_values() {
        // Validate against Table 3.2 from the paper
        // Method II: P95 of χ²_{B-1}, B=10

        let threshold = PsiChiSquareThreshold { alpha: 0.05 };

        // Sample sizes from the paper's table
        let test_cases = [
            (100, 0.169),  // M=100 → ~16.9%
            (200, 0.085),  // M=200 → ~8.5%
            (400, 0.042),  // M=400 → ~4.2%
            (1000, 0.017), // M=1000 → ~1.7%
        ];

        for (sample_size, expected_approx) in test_cases {
            let result = threshold.compute_threshold(sample_size, 10);
            let diff = (result - expected_approx).abs();

            if diff >= 0.005 {
                panic!(
                    "Failed for sample size {}: expected ~{}, got {}, diff={}",
                    sample_size, expected_approx, result, diff
                );
            }
        }
    }

    #[test]
    fn test_degrees_of_freedom_relationship_chi() {
        // Test that B-1 degrees of freedom is correctly applied
        let threshold = PsiChiSquareThreshold { alpha: 0.05 };

        // More bins (higher df) should give larger chi-square critical values
        let bins_5 = threshold.compute_threshold(1000, 5); // 4 df
        let bins_10 = threshold.compute_threshold(1000, 10); // 9 df
        let bins_20 = threshold.compute_threshold(1000, 20); // 19 df

        assert!(
            bins_5 < bins_10,
            "5 bins should give smaller threshold than 10 bins"
        );
        assert!(
            bins_10 < bins_20,
            "10 bins should give smaller threshold than 20 bins"
        );
    }

    #[test]
    fn test_degrees_of_freedom_relationship_normal() {
        let threshold = PsiNormalThreshold { alpha: 0.05 };

        let t_5 = threshold.compute_threshold(1000, 5);
        let t_10 = threshold.compute_threshold(1000, 10);
        let t_20 = threshold.compute_threshold(1000, 20);

        assert!(t_5 < t_10 && t_10 < t_20);
    }

    #[test]
    fn test_alpha_significance_levels_chi() {
        // Test different alpha values (significance levels)
        let sample_size = 1000;
        let bin_count = 10;

        let alpha_01 = PsiChiSquareThreshold { alpha: 0.01 }; // 99th percentile
        let alpha_05 = PsiChiSquareThreshold { alpha: 0.05 }; // 95th percentile
        let alpha_10 = PsiChiSquareThreshold { alpha: 0.10 }; // 90th percentile

        let threshold_99 = alpha_01.compute_threshold(sample_size, bin_count);
        let threshold_95 = alpha_05.compute_threshold(sample_size, bin_count);
        let threshold_90 = alpha_10.compute_threshold(sample_size, bin_count);

        // More conservative (lower alpha) should give higher thresholds
        assert!(
            threshold_99 > threshold_95,
            "99th percentile should be higher than 95th: {} > {}",
            threshold_99,
            threshold_95
        );
        assert!(
            threshold_95 > threshold_90,
            "95th percentile should be higher than 90th: {} > {}",
            threshold_95,
            threshold_90
        );
    }

    #[test]
    fn test_alpha_significance_levels_normal() {
        // Test different alpha values (significance levels)
        let sample_size = 1000;
        let bin_count = 10;

        let alpha_01 = PsiNormalThreshold { alpha: 0.01 }; // 99th percentile
        let alpha_05 = PsiNormalThreshold { alpha: 0.05 }; // 95th percentile
        let alpha_10 = PsiNormalThreshold { alpha: 0.10 }; // 90th percentile

        let threshold_99 = alpha_01.compute_threshold(sample_size, bin_count);
        let threshold_95 = alpha_05.compute_threshold(sample_size, bin_count);
        let threshold_90 = alpha_10.compute_threshold(sample_size, bin_count);

        // More conservative (lower alpha) should give higher thresholds
        assert!(
            threshold_99 > threshold_95,
            "99th percentile should be higher than 95th: {} > {}",
            threshold_99,
            threshold_95
        );
        assert!(
            threshold_95 > threshold_90,
            "95th percentile should be higher than 90th: {} > {}",
            threshold_95,
            threshold_90
        );
    }

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
        let alerts = vec![
            PsiFeatureAlert {
                feature: "feature1".to_string(),
                drift: 0.35,
                threshold: 0.3,
            },
            PsiFeatureAlert {
                feature: "feature2".to_string(),
                drift: 0.45,
                threshold: 0.3,
            },
        ];
        let psi_feature_alerts = PsiFeatureAlerts { alerts };

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
