#[cfg(feature = "sql")]
pub mod drift_executor {

    use crate::error::DriftError;
    use crate::{custom::CustomDrifter, llm::LLMDrifter, psi::PsiDrifter, spc::SpcDrifter};
    use chrono::{DateTime, Utc};

    use scouter_sql::sql::traits::{AlertSqlLogic, ProfileSqlLogic};
    use scouter_sql::{sql::schema::TaskRequest, PostgresClient};
    use scouter_types::DriftProfile;
    use sqlx::{Pool, Postgres};
    use std::collections::BTreeMap;
    use std::result::Result;
    use std::result::Result::Ok;

    use tracing::{debug, error, info, instrument, span, Instrument, Level};

    #[allow(clippy::enum_variant_names)]
    pub enum Drifter {
        SpcDrifter(SpcDrifter),
        PsiDrifter(PsiDrifter),
        CustomDrifter(CustomDrifter),
        LLMDrifter(LLMDrifter),
    }

    impl Drifter {
        pub async fn check_for_alerts(
            &self,
            db_pool: &Pool<Postgres>,
            previous_run: &DateTime<Utc>,
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
                Drifter::LLMDrifter(drifter) => {
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
                DriftProfile::LLM(profile) => Drifter::LLMDrifter(LLMDrifter::new(profile.clone())),
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
            previous_run: &DateTime<Utc>,
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

            info!(
                "Processing drift task for profile: {} and type {}",
                task.uid, task.drift_type
            );

            self.process_task(&task).await?;

            // Update the run dates while still holding the lock
            PostgresClient::update_drift_profile_run_dates(
                &self.db_pool,
                &task.entity_id,
                &task.schedule,
            )
            .instrument(span!(Level::INFO, "Update Run Dates"))
            .await?;

            Ok(Some(task))
        }

        #[instrument(skip_all)]
        async fn process_task(
            &mut self,
            task: &TaskRequest,
            //task: &TaskRequest,
            //task_info: &DriftTaskInfo,
        ) -> Result<(), DriftError> {
            // get the drift profile
            let profile =
                DriftProfile::from_str(&task.drift_type, &task.profile).inspect_err(|e| {
                    error!(
                        "Error converting drift profile for type {:?}: {:?}",
                        &task.drift_type, e
                    );
                })?;

            match self._process_task(profile, &task.previous_run).await {
                Ok(Some(alerts)) => {
                    info!("Drift task processed successfully with alerts");

                    // Insert alerts atomically within the same transaction
                    for alert in alerts {
                        PostgresClient::insert_drift_alert(
                            &self.db_pool,
                            &task.entity_id,
                            alert.get("entity_name").unwrap_or(&"NA".to_string()),
                            &alert,
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
        #[instrument(skip_all)]
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
        use scouter_sql::sql::traits::{EntitySqlLogic, SpcSqlLogic};
        use scouter_sql::PostgresClient;
        use scouter_types::spc::SpcFeatureDriftProfile;
        use scouter_types::{
            spc::{SpcAlertConfig, SpcAlertRule, SpcDriftConfig, SpcDriftProfile},
            AlertDispatchConfig, DriftAlertPaginationRequest,
        };
        use scouter_types::{DriftType, ProfileArgs, SpcRecord};
        use semver::Version;
        use sqlx::{postgres::Postgres, Pool};
        use std::collections::HashMap;

        pub async fn cleanup(pool: &Pool<Postgres>) {
            sqlx::raw_sql(
                r#"
                DELETE
                FROM scouter.spc_drift;

                DELETE
                FROM scouter.drift_entities;

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

            let alert_config = SpcAlertConfig {
                rule: SpcAlertRule::default(),
                // every second for test
                schedule: "* * * * * * *".to_string(),
                features_to_monitor: vec!["col_1".to_string(), "col_3".to_string()],
                dispatch_config: AlertDispatchConfig::default(),
            };

            let config = SpcDriftConfig::new(
                "statworld",
                "test_app",
                "0.1.0",
                Some(true),
                Some(25),
                Some(alert_config),
                None,
            )
            .unwrap();

            let col1_profile = SpcFeatureDriftProfile {
                id: "col_1".to_string(),
                center: -3.997113080300062,
                one_ucl: -1.9742384896265417,
                one_lcl: -6.019987670973582,
                two_ucl: 0.048636101046978464,
                two_lcl: -8.042862261647102,
                three_ucl: 2.071510691720498,
                three_lcl: -10.065736852320622,
                timestamp: Utc::now(),
            };

            let col3_profile = SpcFeatureDriftProfile {
                id: "col_3".to_string(),
                center: -3.937652409303277,
                one_ucl: -2.0275656995100224,
                one_lcl: -5.8477391190965315,
                two_ucl: -0.1174789897167674,
                two_lcl: -7.757825828889787,
                three_ucl: 1.7926077200764872,
                three_lcl: -9.66791253868304,
                timestamp: Utc::now(),
            };

            let drift_profile = DriftProfile::Spc(SpcDriftProfile {
                config,
                features: HashMap::from([
                    (col1_profile.id.clone(), col1_profile),
                    (col3_profile.id.clone(), col3_profile),
                ]),
                scouter_version: "0.1.0".to_string(),
            });

            let profile_args = ProfileArgs {
                space: "statworld".to_string(),
                name: "test_app".to_string(),
                version: Some("0.1.0".to_string()),
                schedule: "* * * * * *".to_string(),
                scouter_version: "0.1.0".to_string(),
                drift_type: DriftType::Spc,
            };

            let version = Version::new(0, 1, 0);
            let uid = PostgresClient::insert_drift_profile(
                &db_pool,
                &drift_profile,
                &profile_args,
                &version,
                &true,
                &true,
            )
            .await
            .unwrap();
            let entity_id = PostgresClient::get_entity_id_from_uid(&db_pool, &uid)
                .await
                .unwrap();

            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

            let mut records = vec![]; // Placeholder for actual records
            for i in 0..100 {
                let record = SpcRecord {
                    // created at + random data
                    created_at: Utc::now() + chrono::Duration::seconds(i),
                    uid: uid.clone(),
                    feature: "col_1".to_string(),
                    value: 10.0 + i as f64,
                };
                records.push(record);
            }

            PostgresClient::insert_spc_drift_records_batch(
                &db_pool,
                &records.iter().collect::<Vec<&SpcRecord>>(),
                &entity_id,
            )
            .await
            .unwrap();

            let mut drift_executor = DriftExecutor::new(&db_pool);
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

            drift_executor.poll_for_tasks().await.unwrap();

            // get alerts from db
            let request = DriftAlertPaginationRequest {
                active: None,
                limit: None,
                uid: uid.clone(),
                ..Default::default()
            };

            let entity_id = PostgresClient::get_entity_id_from_uid(&db_pool, &uid)
                .await
                .unwrap();

            let alerts = PostgresClient::get_paginated_drift_alerts(&db_pool, &request, &entity_id)
                .await
                .unwrap();
            assert!(!alerts.items.is_empty());
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
            let skew_feature = "feature_1";
            let skew_factor = 10;
            let apply_skew = true;
            script = script.replace("{{bin_count}}", &bin_count.to_string());
            script = script.replace("{{skew_feature}}", skew_feature);
            script = script.replace("{{skew_factor}}", &skew_factor.to_string());
            script = script.replace("{{apply_skew}}", &apply_skew.to_string());
            sqlx::raw_sql(&script).execute(&db_pool).await.unwrap();
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

            let mut drift_executor = DriftExecutor::new(&db_pool);

            drift_executor.poll_for_tasks().await.unwrap();

            // get alerts from db
            let request = DriftAlertPaginationRequest {
                uid: "019ae1b4-3003-77c1-8eed-2ec005e85963".to_string(),
                active: None,
                limit: None,
                ..Default::default()
            };

            let entity_id = PostgresClient::get_entity_id_from_space_name_version_drift_type(
                &db_pool,
                "scouter",
                "model",
                "0.1.0",
                DriftType::Psi.to_string(),
            )
            .await
            .unwrap();

            let alerts = PostgresClient::get_paginated_drift_alerts(&db_pool, &request, &entity_id)
                .await
                .unwrap();

            assert_eq!(alerts.items.len(), 1);
        }

        /// This test verifies that the PSI drift executor does **not** generate any drift alerts
        /// when there are **not enough target samples** to meet the minimum threshold required
        /// for PSI calculation.
        ///
        /// This arg determines how many bin counts to simulate for a production environment.
        /// In the script there are 3 features, each with 10 bins.
        /// `bin_count = 2` means we simulate 2 observations per bin.
        /// So for each feature: 10 bins * 2 samples = 20 samples inserted PER insert.
        /// Since the script inserts each feature's data 3 times (simulating 3 production batches),
        /// each feature ends up with: 20 samples * 3 = 60 samples total.
        /// This is below the required threshold of >100 samples per feature for PSI calculation,
        /// so no drift alert should be generated.
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
            let skew_feature = "feature_1";
            let skew_factor = 1;
            let apply_skew = false;
            script = script.replace("{{bin_count}}", &bin_count.to_string());
            script = script.replace("{{skew_feature}}", skew_feature);
            script = script.replace("{{skew_factor}}", &skew_factor.to_string());
            script = script.replace("{{apply_skew}}", &apply_skew.to_string());
            sqlx::raw_sql(&script).execute(&db_pool).await.unwrap();
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

            let mut drift_executor = DriftExecutor::new(&db_pool);

            drift_executor.poll_for_tasks().await.unwrap();

            // get alerts from db
            let request = DriftAlertPaginationRequest {
                uid: "019ae1b4-3003-77c1-8eed-2ec005e85963".to_string(),
                active: None,
                limit: None,
                ..Default::default()
            };

            let entity_id = PostgresClient::get_entity_id_from_space_name_version_drift_type(
                &db_pool,
                "scouter",
                "model",
                "0.1.0",
                DriftType::Psi.to_string(),
            )
            .await
            .unwrap();

            let alerts = PostgresClient::get_paginated_drift_alerts(&db_pool, &request, &entity_id)
                .await
                .unwrap();

            assert!(alerts.items.is_empty());
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
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

            let mut drift_executor = DriftExecutor::new(&db_pool);

            drift_executor.poll_for_tasks().await.unwrap();

            // get alerts from db
            let request = DriftAlertPaginationRequest {
                uid: "scouter|model|0.1.0|custom".to_string(),
                ..Default::default()
            };

            let entity_id = PostgresClient::get_entity_id_from_space_name_version_drift_type(
                &db_pool,
                "scouter",
                "model",
                "0.1.0",
                DriftType::Custom.to_string(),
            )
            .await
            .unwrap();

            let alerts = PostgresClient::get_paginated_drift_alerts(&db_pool, &request, &entity_id)
                .await
                .unwrap();

            assert_eq!(alerts.items.len(), 1);
        }
    }
}
