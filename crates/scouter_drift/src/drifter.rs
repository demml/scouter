#[cfg(feature = "sql")]
pub mod drift_executor {

    use crate::error::DriftError;
    use crate::{custom::CustomDrifter, psi::PsiDrifter, spc::SpcDrifter};
    use chrono::{DateTime, Utc};

    use scouter_sql::sql::traits::{AlertSqlLogic, ProfileSqlLogic};
    use scouter_sql::{sql::schema::TaskRequest, PostgresClient};
    use scouter_types::{DriftProfile, DriftTaskInfo, DriftType};
    use sqlx::{Pool, Postgres};
    use std::collections::BTreeMap;
    use std::result::Result;
    use std::result::Result::Ok;
    use std::str::FromStr;
    use tracing::{debug, error, info, span, Instrument, Level};

    #[allow(clippy::enum_variant_names)]
    pub enum Drifter {
        SpcDrifter(SpcDrifter),
        PsiDrifter(PsiDrifter),
        CustomDrifter(CustomDrifter),
    }

    impl Drifter {
        pub async fn check_for_alerts(
            &self,
            db_pool: &Pool<Postgres>,
            previous_run: DateTime<Utc>,
        ) -> Result<Option<Vec<BTreeMap<String, String>>>, DriftError> {
            match self {
                Drifter::SpcDrifter(drifter) => {
                    drifter.check_for_alerts(db_pool, previous_run).await
                }
                Drifter::PsiDrifter(drifter) => {
                    drifter.check_for_alerts(db_pool, previous_run).await
                }
                Drifter::CustomDrifter(drifter) => {
                    drifter.check_for_alerts(db_pool, previous_run).await
                }
            }
        }
    }

    pub trait GetDrifter {
        fn get_drifter(&self) -> Drifter;
    }

    impl GetDrifter for DriftProfile {
        /// Get a Drifter for processing drift profile tasks
        ///
        /// # Arguments
        ///
        /// * `name` - Name of the drift profile
        /// * `space` - Space of the drift profile
        /// * `version` - Version of the drift profile
        ///
        /// # Returns
        ///
        /// * `Drifter` - Drifter enum
        fn get_drifter(&self) -> Drifter {
            match self {
                DriftProfile::Spc(profile) => Drifter::SpcDrifter(SpcDrifter::new(profile.clone())),
                DriftProfile::Psi(profile) => Drifter::PsiDrifter(PsiDrifter::new(profile.clone())),
                DriftProfile::Custom(profile) => {
                    Drifter::CustomDrifter(CustomDrifter::new(profile.clone()))
                }
            }
        }
    }

    pub struct DriftExecutor {
        db_pool: Pool<Postgres>,
    }

    impl DriftExecutor {
        pub fn new(db_pool: &Pool<Postgres>) -> Self {
            Self {
                db_pool: db_pool.clone(),
            }
        }

        /// Process a single drift computation task
        ///
        /// # Arguments
        ///
        /// * `drift_profile` - Drift profile to compute drift for
        /// * `previous_run` - Previous run timestamp
        /// * `schedule` - Schedule for drift computation
        /// * `transaction` - Postgres transaction
        ///
        /// # Returns
        ///
        pub async fn _process_task(
            &mut self,
            profile: DriftProfile,
            previous_run: DateTime<Utc>,
        ) -> Result<Option<Vec<BTreeMap<String, String>>>, DriftError> {
            // match Drifter enum

            profile
                .get_drifter()
                .check_for_alerts(&self.db_pool, previous_run)
                .await
        }

        async fn do_poll(&mut self) -> Result<Option<TaskRequest>, DriftError> {
            debug!("Polling for drift tasks");

            // Get task from the database (query uses skip lock to pull task and update to processing)
            let task = PostgresClient::get_drift_profile_task(&self.db_pool).await?;

            let Some(task) = task else {
                return Ok(None);
            };

            let task_info = DriftTaskInfo {
                space: task.space.clone(),
                name: task.name.clone(),
                version: task.version.clone(),
                uid: task.uid.clone(),
                drift_type: DriftType::from_str(&task.drift_type).unwrap(),
            };

            info!(
                "Processing drift task for profile: {}/{}/{} and type {}",
                task.space, task.name, task.version, task.drift_type
            );

            self.process_task(&task, &task_info).await?;

            // Update the run dates while still holding the lock
            PostgresClient::update_drift_profile_run_dates(
                &self.db_pool,
                &task_info,
                &task.schedule,
            )
            .instrument(span!(Level::INFO, "Update Run Dates"))
            .await?;

            Ok(Some(task))
        }

        async fn process_task(
            &mut self,
            task: &TaskRequest,
            task_info: &DriftTaskInfo,
        ) -> Result<(), DriftError> {
            // get the drift type
            let drift_type = DriftType::from_str(&task.drift_type).inspect_err(|e| {
                error!("Error converting drift type: {:?}", e);
            })?;

            // get the drift profile
            let profile = DriftProfile::from_str(drift_type.clone(), task.profile.clone())
                .inspect_err(|e| {
                    error!("Error converting drift profile: {:?}", e);
                })?;

            // check for alerts
            match self._process_task(profile, task.previous_run).await {
                Ok(Some(alerts)) => {
                    info!("Drift task processed successfully with alerts");

                    // Insert alerts atomically within the same transaction
                    for alert in alerts {
                        PostgresClient::insert_drift_alert(
                            &self.db_pool,
                            task_info,
                            alert.get("entity_name").unwrap_or(&"NA".to_string()),
                            &alert,
                            &drift_type,
                        )
                        .await
                        .map_err(|e| {
                            error!("Error inserting drift alert: {:?}", e);
                            DriftError::SqlError(e)
                        })?;
                    }
                    Ok(())
                }
                Ok(None) => {
                    info!("Drift task processed successfully with no alerts");
                    Ok(())
                }
                Err(e) => {
                    error!("Error processing drift task: {:?}", e);
                    Err(DriftError::AlertProcessingError(e.to_string()))
                }
            }
        }

        /// Execute single drift computation and alerting
        ///
        /// # Returns
        ///
        /// * `Result<()>` - Result of drift computation and alerting
        pub async fn poll_for_tasks(&mut self) -> Result<(), DriftError> {
            match self.do_poll().await? {
                Some(_) => {
                    info!("Successfully processed drift task");
                    Ok(())
                }
                None => {
                    info!("No triggered schedules found in db. Sleeping for 10 seconds");
                    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                    Ok(())
                }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use rusty_logging::logger::{LogLevel, LoggingConfig, RustyLogger};
        use scouter_settings::DatabaseSettings;
        use scouter_sql::PostgresClient;
        use scouter_types::DriftAlertRequest;
        use sqlx::{postgres::Postgres, Pool};

        pub async fn cleanup(pool: &Pool<Postgres>) {
            sqlx::raw_sql(
                r#"
                DELETE 
                FROM scouter.spc_drift;

                DELETE 
                FROM scouter.observability_metric;

                DELETE
                FROM scouter.custom_drift;

                DELETE
                FROM scouter.drift_alert;

                DELETE
                FROM scouter.drift_profile;

                DELETE
                FROM scouter.psi_drift;
                "#,
            )
            .fetch_all(pool)
            .await
            .unwrap();

            RustyLogger::setup_logging(Some(LoggingConfig::new(
                None,
                Some(LogLevel::Info),
                None,
                None,
            )))
            .unwrap();
        }

        #[tokio::test]
        async fn test_drift_executor_spc() {
            let db_pool = PostgresClient::create_db_pool(&DatabaseSettings::default())
                .await
                .unwrap();

            cleanup(&db_pool).await;

            let mut populate_path =
                std::env::current_dir().expect("Failed to get current directory");
            populate_path.push("src/scripts/populate_spc.sql");

            let script = std::fs::read_to_string(populate_path).unwrap();
            sqlx::raw_sql(&script).execute(&db_pool).await.unwrap();

            let mut drift_executor = DriftExecutor::new(&db_pool);

            drift_executor.poll_for_tasks().await.unwrap();

            // get alerts from db
            let request = DriftAlertRequest {
                space: "statworld".to_string(),
                name: "test_app".to_string(),
                version: "0.1.0".to_string(),
                limit_datetime: None,
                active: None,
                limit: None,
            };
            let alerts = PostgresClient::get_drift_alerts(&db_pool, &request)
                .await
                .unwrap();
            assert!(!alerts.is_empty());
        }

        #[tokio::test]
        async fn test_drift_executor_spc_missing_feature_data() {
            // this tests the scenario where only 1 of 2 features has data in the db when polling
            // for tasks. Need to ensure this does not fail and the present feature and data are
            // still processed
            let db_pool = PostgresClient::create_db_pool(&DatabaseSettings::default())
                .await
                .unwrap();
            cleanup(&db_pool).await;

            let mut populate_path =
                std::env::current_dir().expect("Failed to get current directory");
            populate_path.push("src/scripts/populate_spc_alert.sql");

            let script = std::fs::read_to_string(populate_path).unwrap();
            sqlx::raw_sql(&script).execute(&db_pool).await.unwrap();

            let mut drift_executor = DriftExecutor::new(&db_pool);

            drift_executor.poll_for_tasks().await.unwrap();

            // get alerts from db
            let request = DriftAlertRequest {
                space: "statworld".to_string(),
                name: "test_app".to_string(),
                version: "0.1.0".to_string(),
                limit_datetime: None,
                active: None,
                limit: None,
            };
            let alerts = PostgresClient::get_drift_alerts(&db_pool, &request)
                .await
                .unwrap();

            assert!(!alerts.is_empty());
        }

        #[tokio::test]
        async fn test_drift_executor_psi() {
            let db_pool = PostgresClient::create_db_pool(&DatabaseSettings::default())
                .await
                .unwrap();

            cleanup(&db_pool).await;

            let mut populate_path =
                std::env::current_dir().expect("Failed to get current directory");
            populate_path.push("src/scripts/populate_psi.sql");

            let mut script = std::fs::read_to_string(populate_path).unwrap();
            let bin_count = 1000;
            script = script.replace("{{bin_count}}", &bin_count.to_string());
            sqlx::raw_sql(&script).execute(&db_pool).await.unwrap();

            let mut drift_executor = DriftExecutor::new(&db_pool);

            drift_executor.poll_for_tasks().await.unwrap();

            // get alerts from db
            let request = DriftAlertRequest {
                space: "scouter".to_string(),
                name: "model".to_string(),
                version: "0.1.0".to_string(),
                limit_datetime: None,
                active: None,
                limit: None,
            };
            let alerts = PostgresClient::get_drift_alerts(&db_pool, &request)
                .await
                .unwrap();

            assert!(alerts.len() >= 2);
        }

        #[tokio::test]
        async fn test_drift_executor_psi_not_enough_target_samples() {
            let db_pool = PostgresClient::create_db_pool(&DatabaseSettings::default())
                .await
                .unwrap();

            cleanup(&db_pool).await;

            let mut populate_path =
                std::env::current_dir().expect("Failed to get current directory");
            populate_path.push("src/scripts/populate_psi.sql");

            let mut script = std::fs::read_to_string(populate_path).unwrap();
            let bin_count = 2;
            script = script.replace("{{bin_count}}", &bin_count.to_string());
            sqlx::raw_sql(&script).execute(&db_pool).await.unwrap();

            let mut drift_executor = DriftExecutor::new(&db_pool);

            drift_executor.poll_for_tasks().await.unwrap();

            // get alerts from db
            let request = DriftAlertRequest {
                space: "scouter".to_string(),
                name: "model".to_string(),
                version: "0.1.0".to_string(),
                limit_datetime: None,
                active: None,
                limit: None,
            };
            let alerts = PostgresClient::get_drift_alerts(&db_pool, &request)
                .await
                .unwrap();

            assert!(alerts.is_empty());
        }

        #[tokio::test]
        async fn test_drift_executor_custom() {
            let db_pool = PostgresClient::create_db_pool(&DatabaseSettings::default())
                .await
                .unwrap();

            cleanup(&db_pool).await;

            let mut populate_path =
                std::env::current_dir().expect("Failed to get current directory");
            populate_path.push("src/scripts/populate_custom.sql");

            let script = std::fs::read_to_string(populate_path).unwrap();
            sqlx::raw_sql(&script).execute(&db_pool).await.unwrap();

            let mut drift_executor = DriftExecutor::new(&db_pool);

            drift_executor.poll_for_tasks().await.unwrap();

            // get alerts from db
            let request = DriftAlertRequest {
                space: "scouter".to_string(),
                name: "model".to_string(),
                version: "0.1.0".to_string(),
                limit_datetime: None,
                active: None,
                limit: None,
            };
            let alerts = PostgresClient::get_drift_alerts(&db_pool, &request)
                .await
                .unwrap();

            assert_eq!(alerts.len(), 1);
        }
    }
}
