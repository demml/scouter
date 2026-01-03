use crate::error::DriftError;
use chrono::{DateTime, Utc};
use scouter_dispatch::AlertDispatcher;
use scouter_sql::sql::traits::GenAIDriftSqlLogic;
use scouter_sql::{sql::cache::entity_cache, PostgresClient};
use scouter_types::ProfileBaseArgs;
use scouter_types::{custom::ComparisonMetricAlert, genai::GenAIEvalProfile};
use sqlx::{Pool, Postgres};
use std::collections::BTreeMap;
use tracing::error;
use tracing::info;

pub struct GenAIDrifter {
    profile: GenAIEvalProfile,
}

impl GenAIDrifter {
    pub fn new(profile: GenAIEvalProfile) -> Self {
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

    /// Fetches workflow value from database for the given time limit
    pub async fn get_workflow_value(
        &self,
        limit_datetime: &DateTime<Utc>,
        db_pool: &Pool<Postgres>,
    ) -> Result<Option<f64>, DriftError> {
        let entity_id = entity_cache()
            .get_entity_id_from_uid(db_pool, &self.profile.config.uid)
            .await?;

        PostgresClient::get_genai_workflow_value(db_pool, limit_datetime, &entity_id)
            .await
            .inspect_err(|e| {
                error!(
                    "Unable to obtain genai metric data from DB for {}: {}",
                    self.profile_id(),
                    e
                );
            })
            .map_err(Into::into)
    }

    /// Retrieves metric value and logs if not found
    pub async fn get_metric_value(
        &self,
        limit_datetime: &DateTime<Utc>,
        db_pool: &Pool<Postgres>,
    ) -> Result<Option<f64>, DriftError> {
        let value = self.get_workflow_value(limit_datetime, db_pool).await?;

        if value.is_none() {
            info!(
                "No genai metric data found for {}. Skipping alert processing.",
                self.profile_id()
            );
        }

        Ok(value)
    }

    /// Generates alerts if conditions are met and dispatches them
    pub async fn generate_alerts(
        &self,
        observed_value: f64,
    ) -> Result<Option<BTreeMap<String, String>>, DriftError> {
        // Early return if no alert condition configured
        let Some(alert_condition) = &self.profile.config.alert_config.alert_condition else {
            info!(
                "No alert condition configured for {}. Skipping alert processing.",
                self.profile_id()
            );
            return Ok(None);
        };

        // Check if alert should be triggered
        if !alert_condition.should_alert(observed_value) {
            info!(
                "No alerts to process for {} (observed: {}, baseline: {})",
                self.profile_id(),
                observed_value,
                alert_condition.baseline_value
            );
            return Ok(None);
        }

        // Build comparison alert with owned data
        let metric_name = "genai_workflow_metric".to_string();
        let comparison_alert = ComparisonMetricAlert {
            metric_name: &metric_name,
            baseline_metric: &alert_condition.baseline_value,
            observed_metric: &observed_value,
            delta: &alert_condition.delta,
            alert_threshold: &alert_condition.alert_threshold,
        };

        // Dispatch alert
        let alert_dispatcher = AlertDispatcher::new(&self.profile.config).inspect_err(|e| {
            error!(
                "Error creating alert dispatcher for {}: {}",
                self.profile_id(),
                e
            );
        })?;

        alert_dispatcher
            .process_alerts(&comparison_alert)
            .await
            .inspect_err(|e| {
                error!("Error processing alerts for {}: {}", self.profile_id(), e);
            })?;

        // Convert to owned map before returning
        Ok(Some(comparison_alert.organize_to_map()))
    }

    /// Checks for alerts based on metric value since previous run
    pub async fn check_for_alerts(
        &self,
        db_pool: &Pool<Postgres>,
        previous_run: &DateTime<Utc>,
    ) -> Result<Option<BTreeMap<String, String>>, DriftError> {
        let Some(metric_value) = self.get_metric_value(previous_run, db_pool).await? else {
            return Ok(None);
        };

        self.generate_alerts(metric_value).await.inspect_err(|e| {
            error!("Error generating alerts for {}: {}", self.profile_id(), e);
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use potato_head::mock::{create_score_prompt, LLMTestServer};
    use scouter_types::genai::{ComparisonOperator, GenAIEvalAlertCondition};
    use scouter_types::genai::{
        GenAIAlertConfig, GenAIDriftConfig, GenAIEvalProfile, LLMJudgeTask,
    };
    use scouter_types::{AlertDispatchConfig, ConsoleDispatchConfig};
    use serde_json::Value;

    async fn get_test_drifter() -> GenAIDrifter {
        let prompt = create_score_prompt(Some(vec!["input".to_string()]));

        let task1 = LLMJudgeTask::new_rs(
            "metric1",
            prompt.clone(),
            Value::Number(4.into()),
            None,
            ComparisonOperator::GreaterThanOrEqual,
            None,
            None,
        );

        let task2 = LLMJudgeTask::new_rs(
            "metric2",
            prompt.clone(),
            Value::Number(2.into()),
            None,
            ComparisonOperator::LessThanOrEqual,
            None,
            None,
        );

        let alert_condition = GenAIEvalAlertCondition {
            baseline_value: 5.0,
            alert_threshold: AlertThreshold::Below,
            delta: Some(1.0),
        };
        let alert_config = GenAIAlertConfig {
            schedule: "0 0 * * * *".to_string(),
            dispatch_config: AlertDispatchConfig::Console(ConsoleDispatchConfig { enabled: true }),
            alert_condition: Some(alert_condition),
        };

        let drift_config =
            GenAIDriftConfig::new("scouter", "ML", "0.1.0", 25, alert_config, None).unwrap();

        let profile = GenAIEvalProfile::new(drift_config, None, Some(vec![task1, task2])).unwrap();

        GenAIDrifter::new(profile)
    }

    #[test]
    fn test_generate_alerts_triggers_when_threshold_exceeded() {
        let mut mock = LLMTestServer::new();
        mock.start_server().unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();

        let alerts = runtime.block_on(async {
            let drifter = get_test_drifter().await;

            // Workflow metric value that should trigger an alert
            // (below baseline of 5.0 with delta of 1.0)
            let observed_value = 3.0;

            drifter
                .generate_alerts(observed_value)
                .await
                .expect("Should generate alerts successfully")
        });

        assert!(
            alerts.is_some(),
            "Should generate alerts for out-of-bounds value"
        );

        let alert_map = alerts.unwrap();
        assert!(alert_map.contains_key("metric_name"));
        assert_eq!(
            alert_map.get("metric_name").unwrap(),
            "genai_workflow_metric"
        );

        mock.stop_server().unwrap();
    }

    #[test]
    fn test_generate_alerts_no_trigger_within_threshold() {
        let mut mock = LLMTestServer::new();
        mock.start_server().unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();

        let alerts = runtime.block_on(async {
            let drifter = get_test_drifter().await;

            // Workflow metric value within acceptable range
            let observed_value = 5.0;

            drifter
                .generate_alerts(observed_value)
                .await
                .expect("Should handle no-alert case successfully")
        });

        assert!(
            alerts.is_none(),
            "Should not generate alerts for value within threshold"
        );

        mock.stop_server().unwrap();
    }
}
