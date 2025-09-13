use crate::error::DriftError;
use chrono::{DateTime, Utc};
use scouter_dispatch::AlertDispatcher;
use scouter_sql::sql::traits::LLMDriftSqlLogic;
use scouter_sql::PostgresClient;
use scouter_types::contracts::ServiceInfo;
use scouter_types::{custom::ComparisonMetricAlert, llm::LLMDriftProfile, AlertThreshold};
use sqlx::{Pool, Postgres};
use std::collections::{BTreeMap, HashMap};
use tracing::error;
use tracing::info;

pub struct LLMDrifter {
    service_info: ServiceInfo,
    profile: LLMDriftProfile,
}

impl LLMDrifter {
    pub fn new(profile: LLMDriftProfile) -> Self {
        Self {
            service_info: ServiceInfo {
                name: profile.config.name.clone(),
                space: profile.config.space.clone(),
                version: profile.config.version.clone(),
            },
            profile,
        }
    }

    pub async fn get_observed_llm_metric_values(
        &self,
        limit_datetime: &DateTime<Utc>,
        db_pool: &Pool<Postgres>,
    ) -> Result<HashMap<String, f64>, DriftError> {
        let metrics: Vec<String> = self
            .profile
            .metrics
            .iter()
            .map(|metric| metric.name.clone())
            .collect();

        Ok(PostgresClient::get_llm_metric_values(
            db_pool,
            &self.service_info,
            limit_datetime,
            &metrics,
        )
        .await
        .inspect_err(|e| {
            let msg = format!(
                "Error: Unable to obtain llm metric data from DB for {}/{}/{}: {}",
                self.service_info.space, self.service_info.name, self.service_info.version, e
            );
            error!(msg);
        })?)
    }

    pub async fn get_metric_map(
        &self,
        limit_datetime: &DateTime<Utc>,
        db_pool: &Pool<Postgres>,
    ) -> Result<Option<HashMap<String, f64>>, DriftError> {
        let metric_map = self
            .get_observed_llm_metric_values(limit_datetime, db_pool)
            .await?;

        if metric_map.is_empty() {
            info!(
                "No llm metric data was found for {}/{}/{}. Skipping alert processing.",
                self.service_info.space, self.service_info.name, self.service_info.version,
            );
            return Ok(None);
        }

        Ok(Some(metric_map))
    }

    fn is_out_of_bounds(
        training_value: f64,
        observed_value: f64,
        alert_condition: &AlertThreshold,
        alert_boundary: Option<f64>,
    ) -> bool {
        if observed_value == training_value {
            return false;
        }

        let below_threshold = |boundary: Option<f64>| match boundary {
            Some(b) => observed_value < training_value - b,
            None => observed_value < training_value,
        };

        let above_threshold = |boundary: Option<f64>| match boundary {
            Some(b) => observed_value > training_value + b,
            None => observed_value > training_value,
        };

        match alert_condition {
            AlertThreshold::Below => below_threshold(alert_boundary),
            AlertThreshold::Above => above_threshold(alert_boundary),
            AlertThreshold::Outside => {
                below_threshold(alert_boundary) || above_threshold(alert_boundary)
            } // Handled by early equality check
        }
    }

    pub async fn generate_alerts(
        &self,
        metric_map: &HashMap<String, f64>,
    ) -> Result<Option<Vec<ComparisonMetricAlert>>, DriftError> {
        let metric_alerts: Vec<ComparisonMetricAlert> = metric_map
            .iter()
            .filter_map(|(name, observed_value)| {
                let training_value = self
                    .profile
                    .get_metric_value(name)
                    .inspect_err(|e| {
                        let msg = format!("Error getting training value for metric {name}: {e}");
                        error!(msg);
                    })
                    .ok()?;
                let alert_condition = &self
                    .profile
                    .config
                    .alert_config
                    .alert_conditions
                    .as_ref()
                    .unwrap()[name];
                if Self::is_out_of_bounds(
                    training_value,
                    *observed_value,
                    &alert_condition.alert_threshold,
                    alert_condition.alert_threshold_value,
                ) {
                    Some(ComparisonMetricAlert {
                        metric_name: name.clone(),
                        training_metric_value: training_value,
                        observed_metric_value: *observed_value,
                        alert_threshold_value: alert_condition.alert_threshold_value,
                        alert_threshold: alert_condition.alert_threshold.clone(),
                    })
                } else {
                    None
                }
            })
            .collect();

        if metric_alerts.is_empty() {
            info!(
                "No alerts to process for {}/{}/{}",
                self.service_info.space, self.service_info.name, self.service_info.version
            );
            return Ok(None);
        }

        let alert_dispatcher = AlertDispatcher::new(&self.profile.config).inspect_err(|e| {
            let msg = format!(
                "Error creating alert dispatcher for {}/{}/{}: {}",
                self.service_info.space, self.service_info.name, self.service_info.version, e
            );
            error!(msg);
        })?;

        for alert in &metric_alerts {
            alert_dispatcher
                .process_alerts(alert)
                .await
                .inspect_err(|e| {
                    let msg = format!(
                        "Error processing alerts for {}/{}/{}: {}",
                        self.service_info.space,
                        self.service_info.name,
                        self.service_info.version,
                        e
                    );
                    error!(msg);
                })?;
        }

        Ok(Some(metric_alerts))
    }

    fn organize_alerts(mut alerts: Vec<ComparisonMetricAlert>) -> Vec<BTreeMap<String, String>> {
        let mut alert_vec = Vec::new();
        alerts.iter_mut().for_each(|alert| {
            let mut alert_map = BTreeMap::new();
            alert_map.insert("entity_name".to_string(), alert.metric_name.clone());
            alert_map.insert(
                "training_metric_value".to_string(),
                alert.training_metric_value.to_string(),
            );
            alert_map.insert(
                "observed_metric_value".to_string(),
                alert.observed_metric_value.to_string(),
            );
            let alert_threshold_value_str = match alert.alert_threshold_value {
                Some(value) => value.to_string(),
                None => "None".to_string(),
            };
            alert_map.insert(
                "alert_threshold_value".to_string(),
                alert_threshold_value_str,
            );
            alert_map.insert(
                "alert_threshold".to_string(),
                alert.alert_threshold.to_string(),
            );
            alert_vec.push(alert_map);
        });

        alert_vec
    }

    pub async fn check_for_alerts(
        &self,
        db_pool: &Pool<Postgres>,
        previous_run: DateTime<Utc>,
    ) -> Result<Option<Vec<BTreeMap<String, String>>>, DriftError> {
        let metric_map = self.get_metric_map(&previous_run, db_pool).await?;

        match metric_map {
            Some(metric_map) => {
                let alerts = self.generate_alerts(&metric_map).await.inspect_err(|e| {
                    let msg = format!(
                        "Error generating alerts for {}/{}/{}: {}",
                        self.service_info.space,
                        self.service_info.name,
                        self.service_info.version,
                        e
                    );
                    error!(msg);
                })?;
                match alerts {
                    Some(alerts) => Ok(Some(Self::organize_alerts(alerts))),
                    None => Ok(None),
                }
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use potato_head::{create_score_prompt, LLMTestServer};
    use scouter_types::llm::{LLMAlertConfig, LLMDriftConfig, LLMDriftMetric, LLMDriftProfile};

    fn get_test_drifter() -> LLMDrifter {
        let prompt = create_score_prompt(Some(vec!["input".to_string()]));
        let metric1 = LLMDriftMetric::new(
            "coherence",
            5.0,
            AlertThreshold::Below,
            Some(0.5),
            Some(prompt.clone()),
        )
        .unwrap();

        let metric2 = LLMDriftMetric::new(
            "relevancy",
            5.0,
            AlertThreshold::Below,
            None,
            Some(prompt.clone()),
        )
        .unwrap();

        let alert_config = LLMAlertConfig::default();
        let drift_config =
            LLMDriftConfig::new("scouter", "ML", "0.1.0", 25, alert_config, None).unwrap();

        let profile = LLMDriftProfile::from_metrics(drift_config, vec![metric1, metric2]).unwrap();

        LLMDrifter::new(profile)
    }

    #[test]
    fn test_is_out_of_bounds() {
        // relevancy training value obtained during initial model training.
        let relevancy_training_value = 5.0;

        // observed relevancy metric value captured somewhere after the initial training run.
        let relevancy_observed_value = 4.0;

        // we want relevancy to be as small as possible, so we want to see if the metric has increased.
        let relevancy_alert_condition = AlertThreshold::Below;

        // we do not want to alert if the relevancy values have decreased by more than 0.5
        // if the metric observed has increased beyond (relevancy_training_value - 0.5)
        let relevancy_alert_boundary = Some(0.5);

        let relevancy_is_out_of_bounds = LLMDrifter::is_out_of_bounds(
            relevancy_training_value,
            relevancy_observed_value,
            &relevancy_alert_condition,
            relevancy_alert_boundary,
        );
        assert!(relevancy_is_out_of_bounds);

        // test observed metric has decreased beyond threshold.

        // coherence training value obtained during initial model training.
        let coherence_training_value = 0.76;

        // observed coherence metric value captured somewhere after the initial training run.
        let coherence_observed_value = 0.67;

        // we want to alert if coherence has decreased.
        let coherence_alert_condition = AlertThreshold::Below;

        // we will not be specifying a boundary here as we want to alert if coherence has decreased by any amount
        let coherence_alert_boundary = None;

        let coherence_is_out_of_bounds = LLMDrifter::is_out_of_bounds(
            coherence_training_value,
            coherence_observed_value,
            &coherence_alert_condition,
            coherence_alert_boundary,
        );
        assert!(coherence_is_out_of_bounds);
    }

    #[test]
    fn test_generate_llm_alerts() {
        let mut mock = LLMTestServer::new();
        mock.start_server().unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();

        let drifter = get_test_drifter();

        let mut metric_map = HashMap::new();
        // mse had an initial value of 12.02 when the profile was generated
        metric_map.insert("coherence".to_string(), 4.0);
        // accuracy had an initial 0.75 when the profile was generated
        metric_map.insert("relevancy".to_string(), 4.5);

        let alerts = runtime
            .block_on(async { drifter.generate_alerts(&metric_map).await.unwrap().unwrap() });

        assert_eq!(alerts.len(), 2);
        mock.stop_server().unwrap();
    }
}
