#[cfg(feature = "sql")]
pub mod spc_drifter {
    use crate::spc::alert::generate_alerts;
    use crate::spc::monitor::SpcMonitor;
    use chrono::{DateTime, Utc};
    use ndarray::Array2;
    use ndarray::ArrayView2;
    use scouter_dispatch::AlertDispatcher;
    use scouter_error::DriftError;
    use scouter_sql::sql::traits::SpcSqlLogic;
    use scouter_sql::PostgresClient;
    use scouter_types::contracts::ServiceInfo;
    use scouter_types::spc::{SpcDriftFeatures, SpcDriftProfile, TaskAlerts};
    use sqlx::{Pool, Postgres};
    use std::collections::BTreeMap;
    use tracing::error;
    use tracing::info;

    #[derive(Debug, Clone)]
    pub struct SpcDriftArray {
        pub features: Vec<String>,
        pub array: Array2<f64>,
    }

    impl SpcDriftArray {
        pub fn new(records: SpcDriftFeatures) -> Self {
            let mut features = Vec::new();
            let mut flattened = Vec::new();
            for (feature, drift) in records.features.into_iter() {
                features.push(feature);
                flattened.extend(drift.values);
            }

            let rows = features.len();
            let cols = flattened.len() / rows;
            let array = Array2::from_shape_vec((rows, cols), flattened).unwrap();

            SpcDriftArray { features, array }
        }
    }

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
                    space: profile.config.space.clone(),
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
        /// * `limit_datetime` - Limit timestamp for drift computation (this is the previous_run timestamp)
        /// * `features_to_monitor` - Features to monitor for drift
        ///
        /// # Returns
        ///
        /// * `Result<QueryResult>` - Query result
        async fn get_drift_features(
            &self,
            db_pool: &Pool<Postgres>,
            limit_datetime: &DateTime<Utc>,
            features_to_monitor: &[String],
        ) -> Result<SpcDriftArray, DriftError> {
            let records = PostgresClient::get_spc_drift_records(
                db_pool,
                &self.service_info,
                limit_datetime,
                features_to_monitor,
            )
            .await
            .map_err(|e| DriftError::Error(e.to_string()))?;
            Ok(SpcDriftArray::new(records))
        }

        /// Compute drift for a given drift profile
        ///
        /// # Arguments
        ///
        /// * `limit_datetime` - Limit timestamp for drift computation (this is the previous_run timestamp)
        /// * `db_client` - Postgres client to use for querying feature data
        ///     
        /// # Returns
        ///
        /// * `Result<Array2<f64>>` - Drift array
        pub async fn compute_drift(
            &self,
            limit_datetime: &DateTime<Utc>,
            db_pool: &Pool<Postgres>,
        ) -> Result<(Array2<f64>, Vec<String>), DriftError> {
            let drift_features = self
                .get_drift_features(
                    db_pool,
                    limit_datetime,
                    &self.profile.config.alert_config.features_to_monitor,
                )
                .await?;

            let drift = SpcMonitor::new().calculate_drift_from_sample(
                &drift_features.features,
                &drift_features.array.t().view(), // need to transpose because calculation is done at the row level across each feature
                &self.profile,
            )?;

            Ok((drift, drift_features.features))
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
        pub async fn generate_alerts(
            &self,
            array: &ArrayView2<'_, f64>,
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
                    self.service_info.space, self.service_info.name, self.service_info.version, e
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
                            self.service_info.space,
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
                    self.service_info.space, self.service_info.name, self.service_info.version
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
            db_client: &Pool<Postgres>,
            previous_run: DateTime<Utc>,
        ) -> Result<Option<Vec<BTreeMap<String, String>>>, DriftError> {
            info!(
                "Processing drift task for profile: {}/{}/{}",
                self.service_info.space, self.service_info.name, self.service_info.version
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
                        self.service_info.space,
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
