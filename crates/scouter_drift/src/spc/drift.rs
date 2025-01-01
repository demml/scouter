#[cfg(feature = "sql")]
pub mod spc_drifter {
    use crate::spc::alert::generate_alerts;
    use crate::spc::monitor::SpcMonitor;
    use chrono::NaiveDateTime;
    use ndarray::Array2;
    use ndarray::ArrayView2;
    use scouter_contracts::ServiceInfo;
    use scouter_dispatch::AlertDispatcher;
    use scouter_error::AlertError;
    use scouter_error::DriftError;
    use scouter_sql::{sql::schema::QueryResult, PostgresClient};
    use scouter_types::spc::{SpcDriftProfile, TaskAlerts};
    use std::collections::BTreeMap;
    use tracing::error;
    use tracing::info;

    // Defines the SpcDrifter struct
    // This is used to process drift alerts for spc style profiles
    pub struct SpcDrifter {
        service_info: ServiceInfo,
        profile: SpcDriftProfile,
    }

    impl SpcDrifter {
        pub fn new(profile: SpcDriftProfile) -> Self {
            Self {
                service_info: ServiceInfo {
                    name: profile.config.name.clone(),
                    repository: profile.config.repository.clone(),
                    version: profile.config.version.clone(),
                },
                profile,
            }
        }

        /// Get drift features for a given drift profile
        ///
        /// # Arguments
        ///
        /// * `db_client` - Postgres client to use for querying feature data
        /// * `limit_timestamp` - Limit timestamp for drift computation (this is the previous_run timestamp)
        /// * `features_to_monitor` - Features to monitor for drift
        ///
        /// # Returns
        ///
        /// * `Result<QueryResult>` - Query result
        async fn get_drift_features(
            &self,
            db_client: &PostgresClient,
            limit_timestamp: &str,
            features_to_monitor: &[String],
        ) -> Result<QueryResult, DriftError> {
            let records = db_client
                .get_drift_records(&self.service_info, limit_timestamp, features_to_monitor)
                .await?;
            Ok(records)
        }

        /// Compute drift for a given drift profile
        ///
        /// # Arguments
        ///
        /// * `limit_timestamp` - Limit timestamp for drift computation (this is the previous_run timestamp)
        /// * `db_client` - Postgres client to use for querying feature data
        ///     
        /// # Returns
        ///
        /// * `Result<Array2<f64>>` - Drift array
        pub async fn compute_drift(
            &self,
            limit_timestamp: &NaiveDateTime,
            db_client: &PostgresClient,
        ) -> Result<(Array2<f64>, Vec<String>), DriftError> {
            let drift_features = self
                .get_drift_features(
                    db_client,
                    &limit_timestamp.to_string(),
                    &self.profile.config.alert_config.features_to_monitor,
                )
                .await?;

            let feature_keys: Vec<String> = drift_features.features.keys().cloned().collect();

            let feature_values = drift_features
                .features
                .values()
                .cloned()
                .flat_map(|feature| feature.values.clone())
                .collect::<Vec<_>>();

            // assert all drift features have the same number of values

            let all_same_len = drift_features.features.iter().all(|(_, feature)| {
                feature.values.len()
                    == drift_features
                        .features
                        .values()
                        .next()
                        .unwrap()
                        .values
                        .len()
            });

            if !all_same_len {
                return Err(DriftError::Error(
                    "Feature values are not the same length".to_string(),
                ));
            }

            let num_rows = drift_features.features.len();
            let num_cols = if num_rows > 0 {
                feature_values.len() / num_rows
            } else {
                0
            };

            let nd_feature_arr = Array2::from_shape_vec((num_rows, num_cols), feature_values)
                .map_err(|e| AlertError::DriftError(format!("Error creating ndarray: {}", e)))?;

            let drift = SpcMonitor::new().calculate_drift_from_sample(
                &feature_keys,
                &nd_feature_arr.t().view(), // need to transpose because calculation is done at the row level across each feature
                &self.profile,
            )?;

            Ok((drift, feature_keys))
        }

        /// Generate alerts for a given drift profile
        ///
        /// # Arguments
        ///
        /// * `array` - Drift array
        /// * `features` - Features to monitor for drift
        ///
        /// # Returns
        ///
        /// * `Result<Option<TaskAlerts>>` - Task alerts
        pub async fn generate_alerts<'a>(
            &self,
            array: &ArrayView2<'a, f64>,
            features: &[String],
        ) -> Result<Option<TaskAlerts>, DriftError> {
            let mut task_alerts = TaskAlerts::default();
            // Get alerts
            // keys are the feature names that match the order of the drift array columns
            let alert_rule = self.profile.config.alert_config.rule.clone();
            let alerts = generate_alerts(array, features, &alert_rule)?;

            // Get dispatcher, will default to console if env vars are not found for 3rd party service
            // TODO: Add ability to pass hashmap of kwargs to dispatcher (from drift profile)
            // This would be for things like opsgenie team, feature priority, slack channel, etc.
            let alert_dispatcher = AlertDispatcher::new(&self.profile.config).map_err(|e| {
                error!(
                    "Error creating alert dispatcher for {}/{}/{}: {}",
                    self.service_info.repository,
                    self.service_info.name,
                    self.service_info.version,
                    e
                );
                DriftError::Error("Error creating alert dispatcher".to_string())
            })?;

            if alerts.has_alerts {
                alert_dispatcher
                    .process_alerts(&alerts)
                    .await
                    .map_err(|e| {
                        error!(
                            "Error processing alerts for {}/{}/{}: {}",
                            self.service_info.repository,
                            self.service_info.name,
                            self.service_info.version,
                            e
                        );
                        DriftError::Error("Error processing alerts".to_string())
                    })?;
                task_alerts.alerts = alerts;
                return Ok(Some(task_alerts));
            } else {
                info!(
                    "No alerts to process for {}/{}/{}",
                    self.service_info.repository, self.service_info.name, self.service_info.version
                );
            }

            Ok(None)
        }

        /// organize alerts so that each alert is mapped to a single entry and feature
        /// Some features may produce multiple alerts
        ///
        /// # Arguments
        ///
        /// * `alerts` - TaskAlerts to organize
        ///
        /// # Returns
        ///
        fn organize_alerts(&self, mut alerts: TaskAlerts) -> Vec<BTreeMap<String, String>> {
            let mut tasks = Vec::new();
            alerts.alerts.features.iter_mut().for_each(|(_, feature)| {
                feature.alerts.iter().for_each(|alert| {
                    let alert_map = {
                        let mut alert_map = BTreeMap::new();
                        alert_map.insert("zone".to_string(), alert.zone.clone().to_string());
                        alert_map.insert("kind".to_string(), alert.kind.clone().to_string());
                        alert_map.insert("feature".to_string(), feature.feature.clone());
                        alert_map
                    };
                    tasks.push(alert_map);
                });
            });

            tasks
        }

        /// Process a single drift computation task
        ///
        /// # Arguments
        /// * `previous_run` - Previous run timestamp
        pub async fn check_for_alerts(
            &self,
            db_client: &PostgresClient,
            previous_run: NaiveDateTime,
        ) -> Result<Option<Vec<BTreeMap<String, String>>>, DriftError> {
            info!(
                "Processing drift task for profile: {}/{}/{}",
                self.service_info.repository, self.service_info.name, self.service_info.version
            );

            // Compute drift
            let (drift_array, keys) = self.compute_drift(&previous_run, db_client).await?;

            // if drift array is empty, return early
            if drift_array.is_empty() {
                info!("No features to process returning early");
                return Ok(None);
            }

            // Generate alerts (if any)
            let alerts = self
                .generate_alerts(&drift_array.view(), &keys)
                .await
                .map_err(|e| {
                    error!(
                        "Error generating alerts for {}/{}/{}: {}",
                        self.service_info.repository,
                        self.service_info.name,
                        self.service_info.version,
                        e
                    );
                    DriftError::Error("Error generating alerts".to_string())
                })?;

            match alerts {
                Some(alerts) => {
                    let organized_alerts = self.organize_alerts(alerts);
                    Ok(Some(organized_alerts))
                }
                None => Ok(None),
            }
        }
    }
}
