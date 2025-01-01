use scouter_types::{EveryDay, AlertDispatchType, DriftType, ProfileFuncs, FileName,  DispatchAlertDescription, DispatchDriftConfig, DriftArgs,};
use crate::base::{
    ProfileArgs,
    ProfileBaseArgs, ValidateAlertConfig, MISSING,
};
use scouter_error::{CustomMetricError, ScouterError};
use crate::utils::{json_to_pyobject, pyobject_to_json};
use pyo3::types::PyDict;
use pyo3::{pyclass, pymethods, Bound, Py, PyResult, Python};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::debug;


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
