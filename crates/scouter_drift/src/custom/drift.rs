#[cfg(feature = "sql")]
pub mod custom_drifter {

    use crate::error::DriftError;
    use chrono::{DateTime, Utc};
    use scouter_dispatch::AlertDispatcher;
    use scouter_sql::sql::traits::CustomMetricSqlLogic;
    use scouter_sql::PostgresClient;
    use scouter_types::contracts::ServiceInfo;
    use scouter_types::custom::{AlertThreshold, ComparisonMetricAlert, CustomDriftProfile};
    use sqlx::{Pool, Postgres};
    use std::collections::{BTreeMap, HashMap};
    use tracing::error;
    use tracing::info;

    pub struct CustomDrifter {
        service_info: ServiceInfo,
        profile: CustomDriftProfile,
    }

    impl CustomDrifter {
        pub fn new(profile: CustomDriftProfile) -> Self {
            Self {
                service_info: ServiceInfo {
                    name: profile.config.name.clone(),
                    space: profile.config.space.clone(),
                    version: profile.config.version.clone(),
                },
                profile,
            }
        }

        pub async fn get_observed_custom_metric_values(
            &self,
            limit_datetime: &DateTime<Utc>,
            db_pool: &Pool<Postgres>,
        ) -> Result<HashMap<String, f64>, DriftError> {
            let metrics: Vec<String> = self.profile.metrics.keys().cloned().collect();

            Ok(PostgresClient::get_custom_metric_values(
                db_pool,
                &self.service_info,
                limit_datetime,
                &metrics,
            )
            .await
            .inspect_err(|e| {
                let msg = format!(
                    "Error: Unable to obtain custom metric data from DB for {}/{}/{}: {}",
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
                .get_observed_custom_metric_values(limit_datetime, db_pool)
                .await?;

            if metric_map.is_empty() {
                info!(
                "No custom metric data was found for {}/{}/{}. This indicates that no real-world data values have been recorded in the database for the monitored features as of {}. Skipping alert processing.",
                self.service_info.space,
                self.service_info.name,
                self.service_info.version,
                limit_datetime
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
                    let training_value = self.profile.metrics[name];
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

        fn organize_alerts(
            mut alerts: Vec<ComparisonMetricAlert>,
        ) -> Vec<BTreeMap<String, String>> {
            let mut alert_vec = Vec::new();
            alerts.iter_mut().for_each(|alert| {
                let mut alert_map = BTreeMap::new();
                alert_map.insert("metric_name".to_string(), alert.metric_name.clone());
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
            info!(
                "Processing custom metric(s) task for profile: {}/{}/{}",
                self.service_info.space, self.service_info.name, self.service_info.version
            );

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
        use scouter_types::custom::{
            CustomMetric, CustomMetricAlertConfig, CustomMetricDriftConfig,
        };

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

            let profile = CustomDriftProfile::new(drift_config, custom_metrics, None).unwrap();

            CustomDrifter::new(profile)
        }

        #[test]
        fn test_is_out_of_bounds() {
            // mse training value obtained during initial model training.
            let mse_training_value = 12.0;

            // observed mse metric value captured somewhere after the initial training run.
            let mse_observed_value = 14.5;

            // we want mse to be as small as possible, so we want to see if the metric has increased.
            let mse_alert_condition = AlertThreshold::Above;

            // we do not want to alert if the mse values has simply increased, but we want to alert
            // if the metric observed has increased beyond (mse_training_value + 2.0)
            let mse_alert_boundary = Some(2.0);

            let mse_is_out_of_bounds = CustomDrifter::is_out_of_bounds(
                mse_training_value,
                mse_observed_value,
                &mse_alert_condition,
                mse_alert_boundary,
            );
            assert!(mse_is_out_of_bounds);

            // test observed metric has decreased beyond threshold.

            // accuracy training value obtained during initial model training.
            let accuracy_training_value = 0.76;

            // observed accuracy metric value captured somewhere after the initial training run.
            let accuracy_observed_value = 0.67;

            // we want to alert if accuracy has decreased.
            let accuracy_alert_condition = AlertThreshold::Below;

            // we will not be specifying a boundary here as we want to alert if accuracy has decreased by any amount
            let accuracy_alert_boundary = None;

            let accuracy_is_out_of_bounds = CustomDrifter::is_out_of_bounds(
                accuracy_training_value,
                accuracy_observed_value,
                &accuracy_alert_condition,
                accuracy_alert_boundary,
            );
            assert!(accuracy_is_out_of_bounds);

            // test observed metric has not increased.

            // mae training value obtained during initial model training.
            let mae_training_value = 13.5;

            // observed mae metric value captured somewhere after the initial training run.
            let mae_observed_value = 10.5;

            // we want to alert if mae has increased.
            let mae_alert_condition = AlertThreshold::Above;

            // we will not be specifying a boundary here as we want to alert if mae has increased by any amount
            let mae_alert_boundary = None;

            let mae_is_out_of_bounds = CustomDrifter::is_out_of_bounds(
                mae_training_value,
                mae_observed_value,
                &mae_alert_condition,
                mae_alert_boundary,
            );
            assert!(!mae_is_out_of_bounds);
        }

        #[tokio::test]
        async fn test_generate_alerts() {
            let drifter = get_test_drifter();

            let mut metric_map = HashMap::new();
            // mse had an initial value of 12.02 when the profile was generated
            metric_map.insert("mse".to_string(), 14.0);
            // accuracy had an initial 0.75 when the profile was generated
            metric_map.insert("accuracy".to_string(), 0.65);

            let alerts = drifter.generate_alerts(&metric_map).await.unwrap().unwrap();

            assert_eq!(alerts.len(), 2);
        }
    }
}
