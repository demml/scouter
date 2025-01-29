#[cfg(feature = "sql")]
pub mod drift_executor {

    use scouter_contracts::ServiceInfo;
    use scouter_error::DriftError;
    use scouter_sql::{sql::schema::TaskRequest, PostgresClient};
    use sqlx::{Postgres, Transaction};

    use crate::{custom::CustomDrifter, psi::PsiDrifter, spc::SpcDrifter};
    use chrono::NaiveDateTime;
    use scouter_types::{DriftProfile, DriftType};
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
            db_client: &PostgresClient,
            previous_run: NaiveDateTime,
        ) -> Result<Option<Vec<BTreeMap<String, String>>>, DriftError> {
            match self {
                Drifter::SpcDrifter(drifter) => drifter
                    .check_for_alerts(db_client, previous_run)
                    .await
                    .map_err(DriftError::from),
                Drifter::PsiDrifter(drifter) => drifter
                    .check_for_alerts(db_client, previous_run)
                    .await
                    .map_err(DriftError::from),
                Drifter::CustomDrifter(drifter) => drifter
                    .check_for_alerts(db_client, previous_run)
                    .await
                    .map_err(DriftError::from),
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
        /// * `repository` - Repository of the drift profile
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
        db_client: PostgresClient,
    }

    impl DriftExecutor {
        pub fn new(db_client: PostgresClient) -> Self {
            Self { db_client }
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
            previous_run: NaiveDateTime,
        ) -> Result<Option<Vec<BTreeMap<String, String>>>, DriftError> {
            // match Drifter enum

            profile
                .get_drifter()
                .check_for_alerts(&self.db_client, previous_run)
                .await
        }

        async fn do_poll(
            &mut self,
        ) -> Result<(Option<TaskRequest>, Transaction<'static, Postgres>), DriftError> {
            debug!("Polling for drift tasks");
            let mut transaction = self.db_client.pool.begin().await.map_err(|e| {
                error!("Error starting transaction: {:?}", e);
                DriftError::Error(e.to_string())
            })?;

            // this will pull a drift profile from the db
            let task = match PostgresClient::get_drift_profile_task(&mut transaction).await {
                Ok(task) => task,
                Err(e) => {
                    error!("Error getting drift profile task: {:?}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                    None
                }
            };

            Ok((task, transaction))
        }

        async fn process_task(
            &mut self,
            task: &TaskRequest,
            service_info: &ServiceInfo,
        ) -> Result<(), DriftError> {
            match DriftType::from_str(&task.drift_type) {
                // match drift_profile
                Ok(drift_type) => match DriftProfile::from_str(drift_type, task.profile.clone()) {
                    // process drift profile task
                    Ok(profile) => match self._process_task(profile, task.previous_run).await {
                        // check for alerts
                        Ok(alerts) => {
                            info!("Drift task processed successfully");

                            if let Some(alerts) = alerts {
                                // insert each task into db
                                for alert in alerts {
                                    if let Err(e) = self
                                        .db_client
                                        .insert_drift_alert(
                                            service_info,
                                            alert.get("feature").unwrap_or(&"NA".to_string()),
                                            &alert,
                                        )
                                        .await
                                    {
                                        error!("Error inserting drift alerts: {:?}", e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("Error processing drift task: {:?}", e);
                        }
                    },
                    Err(e) => {
                        error!("Error converting drift profile: {:?}", e);
                    }
                },
                Err(e) => {
                    error!("Error converting drift type: {:?}", e);
                }
            }

            Ok(())
        }

        /// Execute single drift computation and alerting
        ///
        /// # Returns
        ///
        /// * `Result<()>` - Result of drift computation and alerting

        pub async fn poll_for_tasks(&mut self) -> Result<(), DriftError> {
            // this will pull a drift profile from the db
            let (task, mut trx) = self.do_poll().await?;

            let Some(task) = task else {
                trx.commit()
                    .await
                    .map_err(|e| DriftError::Error(e.to_string()))?;
                info!("No triggered schedules found in db. Sleeping for 10 seconds");
                tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                return Ok(());
            };

            let service_info = ServiceInfo {
                repository: task.repository.clone(),
                name: task.name.clone(),
                version: task.version.clone(),
            };

            self.process_task(&task, &service_info)
                .instrument(span!(Level::INFO, "Process Task"))
                .await?;

            if let Err(e) = PostgresClient::update_drift_profile_run_dates(
                &mut trx,
                &service_info,
                &task.schedule,
            )
            .instrument(span!(Level::INFO, "Upate Run Dates"))
            .await
            {
                error!("Error updating drift profile run dates: {:?}", e);
            } else {
                info!("Drift profile run dates updated successfully");
            }

            trx.commit().await.map_err(|e| {
                error!("Error committing transaction: {:?}", e);
                DriftError::Error(e.to_string())
            })?;

            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use rusty_logging::logger::{LogLevel, LoggingConfig, RustyLogger};
        use scouter_contracts::DriftAlertRequest;
        use scouter_sql::PostgresClient;

        use sqlx::{postgres::Postgres, Pool};

        pub async fn cleanup(pool: &Pool<Postgres>) {
            sqlx::raw_sql(
                r#"
                DELETE 
                FROM scouter.drift;

                DELETE 
                FROM scouter.observability_metrics;

                DELETE
                FROM scouter.custom_metrics;

                DELETE
                FROM scouter.drift_alerts;

                DELETE
                FROM scouter.drift_profile;

                DELETE
                FROM scouter.observed_bin_count;
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
            let client = PostgresClient::new(None, None).await.unwrap();
            cleanup(&client.pool).await;

            let mut populate_path =
                std::env::current_dir().expect("Failed to get current directory");
            populate_path.push("src/scripts/populate_spc.sql");

            let script = std::fs::read_to_string(populate_path).unwrap();
            sqlx::raw_sql(&script).execute(&client.pool).await.unwrap();

            let mut drift_executor = DriftExecutor::new(client.clone());

            drift_executor.poll_for_tasks().await.unwrap();

            // get alerts from db
            let request = DriftAlertRequest {
                repository: "statworld".to_string(),
                name: "test_app".to_string(),
                version: "0.1.0".to_string(),
                limit_datetime: None,
                active: None,
                limit: None,
            };
            let alerts = client.get_drift_alerts(&request).await.unwrap();

            assert!(!alerts.is_empty());
        }

        #[tokio::test]
        async fn test_drift_executor_psi() {
            let client = PostgresClient::new(None, None).await.unwrap();
            cleanup(&client.pool).await;

            let mut populate_path =
                std::env::current_dir().expect("Failed to get current directory");
            populate_path.push("src/scripts/populate_psi.sql");

            let script = std::fs::read_to_string(populate_path).unwrap();
            sqlx::raw_sql(&script).execute(&client.pool).await.unwrap();

            let mut drift_executor = DriftExecutor::new(client.clone());

            drift_executor.poll_for_tasks().await.unwrap();

            // get alerts from db
            let request = DriftAlertRequest {
                repository: "scouter".to_string(),
                name: "model".to_string(),
                version: "0.1.0".to_string(),
                limit_datetime: None,
                active: None,
                limit: None,
            };
            let alerts = client.get_drift_alerts(&request).await.unwrap();

            assert!(alerts.len() >= 2);
        }

        #[tokio::test]
        async fn test_drift_executor_custom() {
            let client = PostgresClient::new(None, None).await.unwrap();
            cleanup(&client.pool).await;

            let mut populate_path =
                std::env::current_dir().expect("Failed to get current directory");
            populate_path.push("src/scripts/populate_custom.sql");

            let script = std::fs::read_to_string(populate_path).unwrap();
            sqlx::raw_sql(&script).execute(&client.pool).await.unwrap();

            let mut drift_executor = DriftExecutor::new(client.clone());

            drift_executor.poll_for_tasks().await.unwrap();

            // get alerts from db
            let request = DriftAlertRequest {
                repository: "scouter".to_string(),
                name: "model".to_string(),
                version: "0.1.0".to_string(),
                limit_datetime: None,
                active: None,
                limit: None,
            };
            let alerts = client.get_drift_alerts(&request).await.unwrap();

            assert_eq!(alerts.len(), 1);
        }
    }
}
