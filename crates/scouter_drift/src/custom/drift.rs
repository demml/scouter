use crate::error::DriftError;
use chrono::{DateTime, Utc};
use scouter_dispatch::AlertDispatcher;
use scouter_sql::sql::traits::CustomMetricSqlLogic;
use scouter_sql::{sql::cache::entity_cache, PostgresClient};
use scouter_types::custom::{ComparisonMetricAlert, CustomDriftProfile};
use scouter_types::ProfileBaseArgs;
use sqlx::{Pool, Postgres};
use std::collections::{BTreeMap, HashMap};
use tracing::error;
use tracing::info;
pub struct CustomDrifter {
    profile: CustomDriftProfile,
}

impl CustomDrifter {
    pub fn new(profile: CustomDriftProfile) -> Self {
        Self { profile }
    }

    /// Helper method to format profile identifier for logging
    fn profile_id(&self) -> String {
        format!(
            "{}/{}/{}",
            self.profile.space(),
            self.profile.name(),
            self.profile.version()
        )
    }

    /// Fetches observed custom metric values from database for the given time limit
    pub async fn get_observed_custom_metric_values(
        &self,
        limit_datetime: &DateTime<Utc>,
        db_pool: &Pool<Postgres>,
    ) -> Result<HashMap<String, f64>, DriftError> {
        let metrics: Vec<String> = self.profile.metrics.keys().cloned().collect();
        let entity_id = entity_cache()
            .get_entity_id_from_uid(db_pool, &self.profile.config.uid)
            .await?;

        PostgresClient::get_custom_metric_values(db_pool, limit_datetime, &metrics, &entity_id)
            .await
            .inspect_err(|e| {
                error!(
                    "Unable to obtain custom metric data from DB for {}: {}",
                    self.profile_id(),
                    e
                );
            })
            .map_err(Into::into)
    }

    /// Retrieves metric map and logs if no data found
    pub async fn get_metric_map(
        &self,
        limit_datetime: &DateTime<Utc>,
        db_pool: &Pool<Postgres>,
    ) -> Result<Option<HashMap<String, f64>>, DriftError> {
        let metric_map = self
            .get_observed_custom_metric_values(limit_datetime, db_pool)
            .await?;

        if metric_map.is_empty() {
            info!(
                "No custom metric data found for {}. Skipping alert processing.",
                self.profile_id()
            );
            return Ok(None);
        }

        Ok(Some(metric_map))
    }

    /// Generates alerts for metrics that exceed their thresholds and dispatches them
    pub async fn generate_alerts(
        &self,
        metric_map: &HashMap<String, f64>,
    ) -> Result<Option<Vec<BTreeMap<String, String>>>, DriftError> {
        // Early return if no alert conditions configured
        let Some(alert_conditions) = &self.profile.config.alert_config.alert_conditions else {
            info!(
                "No alert conditions configured for {}. Skipping alert processing.",
                self.profile_id()
            );
            return Ok(None);
        };

        // Collect metrics that should trigger alerts
        let metric_alerts: Vec<ComparisonMetricAlert> = metric_map
            .iter()
            .filter_map(|(name, observed_value)| {
                let alert_condition = alert_conditions.get(name)?;

                if alert_condition.should_alert(*observed_value) {
                    Some(ComparisonMetricAlert {
                        metric_name: name,
                        baseline_metric: &alert_condition.baseline_value,
                        observed_metric: observed_value,
                        delta: &alert_condition.delta,
                        alert_threshold: &alert_condition.alert_threshold,
                    })
                } else {
                    None
                }
            })
            .collect();

        // Early return if no alerts to process
        if metric_alerts.is_empty() {
            info!(
                "No alerts to process for {} (checked {} metrics)",
                self.profile_id(),
                metric_map.len()
            );
            return Ok(None);
        }

        // Dispatch alerts
        let alert_dispatcher = AlertDispatcher::new(&self.profile.config).inspect_err(|e| {
            error!(
                "Error creating alert dispatcher for {}: {}",
                self.profile_id(),
                e
            );
        })?;

        for alert in &metric_alerts {
            alert_dispatcher
                .process_alerts(alert)
                .await
                .inspect_err(|e| {
                    error!(
                        "Error processing alert for metric '{}' in {}: {}",
                        alert.metric_name,
                        self.profile_id(),
                        e
                    );
                })?;
        }

        // Convert to owned maps before returning
        Ok(Some(
            metric_alerts
                .iter()
                .map(|alert| alert.organize_to_map())
                .collect(),
        ))
    }

    /// Checks for alerts based on metric values since previous run
    pub async fn check_for_alerts(
        &self,
        db_pool: &Pool<Postgres>,
        previous_run: &DateTime<Utc>,
    ) -> Result<Option<Vec<BTreeMap<String, String>>>, DriftError> {
        let Some(metric_map) = self.get_metric_map(previous_run, db_pool).await? else {
            return Ok(None);
        };

        self.generate_alerts(&metric_map).await.inspect_err(|e| {
            error!("Error generating alerts for {}: {}", self.profile_id(), e);
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scouter_types::custom::{CustomMetric, CustomMetricAlertConfig, CustomMetricDriftConfig};
    use scouter_types::AlertThreshold;

    fn get_test_drifter() -> CustomDrifter {
        let custom_metrics = vec![
            CustomMetric::new("mse", 12.02, AlertThreshold::Above, Some(1.0)).unwrap(),
            CustomMetric::new("accuracy", 0.75, AlertThreshold::Below, None).unwrap(),
        ];

        let drift_config = CustomMetricDriftConfig::new(
            "scouter",
            "model",
            "0.1.0",
            25,
            CustomMetricAlertConfig::default(),
            None,
        )
        .unwrap();

        let profile = CustomDriftProfile::new(drift_config, custom_metrics).unwrap();

        CustomDrifter::new(profile)
    }

    #[tokio::test]
    async fn test_generate_alerts_triggers_for_out_of_bounds_metrics() {
        let drifter = get_test_drifter();

        let mut metric_map = HashMap::new();
        // mse baseline: 12.02, threshold: Above, delta: 1.0
        // This should trigger (14.0 > 12.02 + 1.0)
        metric_map.insert("mse".to_string(), 14.0);
        // accuracy baseline: 0.75, threshold: Below, delta: None
        // This should trigger (0.65 < 0.75)
        metric_map.insert("accuracy".to_string(), 0.65);

        let alerts = drifter
            .generate_alerts(&metric_map)
            .await
            .expect("Should generate alerts successfully")
            .expect("Should have alerts");

        assert_eq!(alerts.len(), 2, "Should generate 2 alerts");

        // Verify alert contents
        let has_mse_alert = alerts
            .iter()
            .any(|a| a.get("entity_name") == Some(&"mse".to_string()));
        let has_accuracy_alert = alerts
            .iter()
            .any(|a| a.get("entity_name") == Some(&"accuracy".to_string()));

        assert!(has_mse_alert, "Should have MSE alert");
        assert!(has_accuracy_alert, "Should have accuracy alert");
    }

    #[tokio::test]
    async fn test_generate_alerts_no_trigger_within_threshold() {
        let drifter = get_test_drifter();

        let mut metric_map = HashMap::new();
        // mse baseline: 12.02, threshold: Above, delta: 1.0
        // This should NOT trigger (12.5 < 12.02 + 1.0)
        metric_map.insert("mse".to_string(), 12.5);
        // accuracy baseline: 0.75, threshold: Below
        // This should NOT trigger (0.76 > 0.75)
        metric_map.insert("accuracy".to_string(), 0.76);

        let alerts = drifter
            .generate_alerts(&metric_map)
            .await
            .expect("Should handle no-alert case successfully");

        assert!(
            alerts.is_none(),
            "Should not generate alerts for values within threshold"
        );
    }

    #[tokio::test]
    async fn test_generate_alerts_partial_triggers() {
        let drifter = get_test_drifter();

        let mut metric_map = HashMap::new();
        // Only MSE should trigger
        metric_map.insert("mse".to_string(), 14.0);
        // Accuracy within bounds
        metric_map.insert("accuracy".to_string(), 0.76);

        let alerts = drifter
            .generate_alerts(&metric_map)
            .await
            .expect("Should generate alerts successfully")
            .expect("Should have alerts");

        assert_eq!(alerts.len(), 1, "Should generate 1 alert");
        assert_eq!(
            alerts[0].get("entity_name"),
            Some(&"mse".to_string()),
            "Alert should be for MSE metric"
        );
    }

    #[test]
    fn test_profile_id_formatting() {
        let drifter = get_test_drifter();
        let profile_id = drifter.profile_id();

        assert_eq!(profile_id, "scouter/model/0.1.0");
    }
}
