use crate::sql::traits::{
    AlertSqlLogic, ArchiveSqlLogic, CustomMetricSqlLogic, ObservabilitySqlLogic, ProfileSqlLogic,
    PsiSqlLogic, SpcSqlLogic, UserSqlLogic,
};

use scouter_error::{ScouterError, SqlError};
use scouter_settings::DatabaseSettings;

use scouter_types::{RecordType, ServerRecords, ToDriftRecords};

use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::result::Result::Ok;
use tracing::{debug, error, info, instrument};

// TODO: Explore refactoring and breaking this out into multiple client types (i.e., spc, psi, etc.)
// Postgres client is one of the lowest-level abstractions so it may not be worth it, as it could make server logic annoying. Worth exploring though.

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PostgresClient {}

impl SpcSqlLogic for PostgresClient {}
impl CustomMetricSqlLogic for PostgresClient {}
impl PsiSqlLogic for PostgresClient {}
impl UserSqlLogic for PostgresClient {}
impl ProfileSqlLogic for PostgresClient {}
impl ObservabilitySqlLogic for PostgresClient {}
impl AlertSqlLogic for PostgresClient {}
impl ArchiveSqlLogic for PostgresClient {}

impl PostgresClient {
    /// Setup the application with the given database pool.
    ///
    /// # Returns
    ///
    /// * `Result<Pool<Postgres>, anyhow::Error>` - Result of the database pool
    #[instrument(skip(database_settings))]
    pub async fn create_db_pool(
        database_settings: &DatabaseSettings,
    ) -> Result<Pool<Postgres>, SqlError> {
        let pool = match PgPoolOptions::new()
            .max_connections(database_settings.max_connections)
            .connect(&database_settings.connection_uri)
            .await
        {
            Ok(pool) => {
                info!("✅ Successfully connected to database");
                pool
            }
            Err(err) => {
                error!("🚨 Failed to connect to database {:?}", err);
                std::process::exit(1);
            }
        };

        // Run migrations
        if let Err(err) = Self::run_migrations(&pool).await {
            error!("🚨 Failed to run migrations {:?}", err);
            std::process::exit(1);
        }

        Ok(pool)
    }

    pub async fn run_migrations(pool: &Pool<Postgres>) -> Result<(), SqlError> {
        info!("Running migrations");
        sqlx::migrate!("src/migrations")
            .run(pool)
            .await
            .map_err(|e| SqlError::MigrationError(format!("{}", e)))?;

        debug!("Migrations complete");

        Ok(())
    }
}

pub struct MessageHandler {}

impl MessageHandler {
    #[instrument(skip(records), name = "Insert Server Records")]
    pub async fn insert_server_records(
        pool: &Pool<Postgres>,
        records: &ServerRecords,
    ) -> Result<(), ScouterError> {
        match records.record_type()? {
            RecordType::Spc => {
                debug!("SPC record count: {:?}", records.len());
                let records = records.to_spc_drift_records()?;
                for record in records.iter() {
                    let _ = PostgresClient::insert_spc_drift_record(pool, record)
                        .await
                        .map_err(|e| {
                            error!("Failed to insert drift record: {:?}", e);
                        });
                }
            }
            RecordType::Observability => {
                debug!("Observability record count: {:?}", records.len());
                let records = records.to_observability_drift_records()?;
                for record in records.iter() {
                    let _ = PostgresClient::insert_observability_record(pool, record)
                        .await
                        .map_err(|e| {
                            error!("Failed to insert observability record: {:?}", e);
                        });
                }
            }
            RecordType::Psi => {
                debug!("PSI record count: {:?}", records.len());
                let records = records.to_psi_drift_records()?;
                for record in records.iter() {
                    let _ = PostgresClient::insert_bin_counts(pool, record)
                        .await
                        .map_err(|e| {
                            error!("Failed to insert bin count record: {:?}", e);
                        });
                }
            }
            RecordType::Custom => {
                debug!("Custom record count: {:?}", records.len());
                let records = records.to_custom_metric_drift_records()?;
                for record in records.iter() {
                    let _ = PostgresClient::insert_custom_metric_value(pool, record)
                        .await
                        .map_err(|e| {
                            error!("Failed to insert bin count record: {:?}", e);
                        });
                }
            }
        };
        Ok(())
    }
}

/// Runs database integratino tests
/// Note - binned queries targeting custom intervals with long-term and short-term data are
/// done in the scouter-server integration tests
#[cfg(test)]
mod tests {

    use super::*;
    use crate::sql::schema::User;
    use chrono::Utc;
    use rand::Rng;
    use scouter_contracts::{
        DriftAlertRequest, DriftRequest, GetProfileRequest, ProfileStatusRequest, ServiceInfo,
    };
    use scouter_settings::ObjectStorageSettings;
    use scouter_types::spc::SpcDriftProfile;
    use scouter_types::*;
    use std::collections::BTreeMap;
    const SPACE: &str = "space";
    const NAME: &str = "name";
    const VERSION: &str = "1.0.0";

    pub async fn cleanup(pool: &Pool<Postgres>) {
        sqlx::raw_sql(
            r#"
            DELETE
            FROM scouter.spc_drift;

            DELETE
            FROM scouter.observability_metric;

            DELETE
            FROM scouter.custom_metric;

            DELETE
            FROM scouter.drift_alert;

            DELETE
            FROM scouter.drift_profile;

            DELETE
            FROM scouter.observed_bin_count;

             DELETE
            FROM scouter.user;
            "#,
        )
        .fetch_all(pool)
        .await
        .unwrap();
    }

    pub async fn db_pool() -> Pool<Postgres> {
        let pool = PostgresClient::create_db_pool(&DatabaseSettings::default())
            .await
            .unwrap();

        cleanup(&pool).await;

        pool
    }

    #[tokio::test]
    async fn test_postgres() {
        let _pool = db_pool().await;
    }

    #[tokio::test]
    async fn test_postgres_drift_alert() {
        let pool = db_pool().await;

        let timestamp = Utc::now();

        for _ in 0..10 {
            let service_info = ServiceInfo {
                space: SPACE.to_string(),
                name: NAME.to_string(),
                version: VERSION.to_string(),
            };

            let alert = (0..10)
                .map(|i| (i.to_string(), i.to_string()))
                .collect::<BTreeMap<String, String>>();

            let result = PostgresClient::insert_drift_alert(
                &pool,
                &service_info,
                "test",
                &alert,
                &DriftType::Spc,
            )
            .await
            .unwrap();

            assert_eq!(result.rows_affected(), 1);
        }

        // get alerts
        let alert_request = DriftAlertRequest {
            space: SPACE.to_string(),
            name: NAME.to_string(),
            version: VERSION.to_string(),
            active: Some(true),
            limit: None,
            limit_datetime: None,
        };

        let alerts = PostgresClient::get_drift_alerts(&pool, &alert_request)
            .await
            .unwrap();
        assert!(alerts.len() > 5);

        // get alerts limit 1
        let alert_request = DriftAlertRequest {
            space: SPACE.to_string(),
            name: NAME.to_string(),
            version: VERSION.to_string(),
            active: Some(true),
            limit: Some(1),
            limit_datetime: None,
        };

        let alerts = PostgresClient::get_drift_alerts(&pool, &alert_request)
            .await
            .unwrap();
        assert_eq!(alerts.len(), 1);

        // get alerts limit timestamp
        let alert_request = DriftAlertRequest {
            space: SPACE.to_string(),
            name: NAME.to_string(),
            version: VERSION.to_string(),
            active: Some(true),
            limit: None,
            limit_datetime: Some(timestamp),
        };

        let alerts = PostgresClient::get_drift_alerts(&pool, &alert_request)
            .await
            .unwrap();
        assert!(alerts.len() > 5);
    }

    #[tokio::test]
    async fn test_postgres_spc_drift_record() {
        let pool = db_pool().await;

        let record = SpcServerRecord {
            created_at: Utc::now(),
            space: SPACE.to_string(),
            name: NAME.to_string(),
            version: VERSION.to_string(),
            feature: "test".to_string(),
            value: 1.0,
        };

        let result = PostgresClient::insert_spc_drift_record(&pool, &record)
            .await
            .unwrap();

        assert_eq!(result.rows_affected(), 1);
    }

    #[tokio::test]
    async fn test_postgres_bin_count() {
        let pool = db_pool().await;

        let record = PsiServerRecord {
            created_at: Utc::now(),
            space: SPACE.to_string(),
            name: NAME.to_string(),
            version: VERSION.to_string(),
            feature: "test".to_string(),
            bin_id: 1,
            bin_count: 1,
        };

        let result = PostgresClient::insert_bin_counts(&pool, &record)
            .await
            .unwrap();

        assert_eq!(result.rows_affected(), 1);
    }

    #[tokio::test]
    async fn test_postgres_observability_record() {
        let pool = db_pool().await;

        let record = ObservabilityMetrics::default();

        let result = PostgresClient::insert_observability_record(&pool, &record)
            .await
            .unwrap();

        assert_eq!(result.rows_affected(), 1);
    }

    #[tokio::test]
    async fn test_postgres_crud_drift_profile() {
        let pool = db_pool().await;

        let mut spc_profile = SpcDriftProfile::default();

        let result =
            PostgresClient::insert_drift_profile(&pool, &DriftProfile::Spc(spc_profile.clone()))
                .await
                .unwrap();

        assert_eq!(result.rows_affected(), 1);

        spc_profile.scouter_version = "test".to_string();

        let result =
            PostgresClient::update_drift_profile(&pool, &DriftProfile::Spc(spc_profile.clone()))
                .await
                .unwrap();

        assert_eq!(result.rows_affected(), 1);

        let profile = PostgresClient::get_drift_profile(
            &pool,
            &GetProfileRequest {
                name: spc_profile.config.name.clone(),
                space: spc_profile.config.space.clone(),
                version: spc_profile.config.version.clone(),
                drift_type: DriftType::Spc,
            },
        )
        .await
        .unwrap();

        let deserialized = serde_json::from_value::<SpcDriftProfile>(profile.unwrap()).unwrap();

        assert_eq!(deserialized, spc_profile);

        PostgresClient::update_drift_profile_status(
            &pool,
            &ProfileStatusRequest {
                name: spc_profile.config.name.clone(),
                space: spc_profile.config.space.clone(),
                version: spc_profile.config.version.clone(),
                active: false,
                drift_type: Some(DriftType::Spc),
            },
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_postgres_get_features() {
        let pool = db_pool().await;

        let timestamp = Utc::now();

        for _ in 0..10 {
            for j in 0..10 {
                let record = SpcServerRecord {
                    created_at: Utc::now(),
                    space: SPACE.to_string(),
                    name: NAME.to_string(),
                    version: VERSION.to_string(),
                    feature: format!("test{}", j),
                    value: j as f64,
                };

                let result = PostgresClient::insert_spc_drift_record(&pool, &record)
                    .await
                    .unwrap();
                assert_eq!(result.rows_affected(), 1);
            }
        }

        let service_info = ServiceInfo {
            space: SPACE.to_string(),
            name: NAME.to_string(),
            version: VERSION.to_string(),
        };

        let features = PostgresClient::get_spc_features(&pool, &service_info)
            .await
            .unwrap();
        assert_eq!(features.len(), 10);

        let records =
            PostgresClient::get_spc_drift_records(&pool, &service_info, &timestamp, &features)
                .await
                .unwrap();

        assert_eq!(records.features.len(), 10);

        let binned_records = PostgresClient::get_binned_spc_drift_records(
            &pool,
            &DriftRequest {
                space: SPACE.to_string(),
                name: NAME.to_string(),
                version: VERSION.to_string(),
                time_interval: TimeInterval::FiveMinutes,
                max_data_points: 10,
                drift_type: DriftType::Spc,
                ..Default::default()
            },
            &DatabaseSettings::default().retention_period,
            &ObjectStorageSettings::default(),
        )
        .await
        .unwrap();

        assert_eq!(binned_records.features.len(), 10);
    }

    #[tokio::test]
    async fn test_postgres_bin_proportions() {
        let pool = db_pool().await;

        let timestamp = Utc::now();

        for feature in 0..3 {
            for bin in 0..=5 {
                for _ in 0..=100 {
                    let record = PsiServerRecord {
                        created_at: Utc::now(),
                        space: SPACE.to_string(),
                        name: NAME.to_string(),
                        version: VERSION.to_string(),
                        feature: format!("feature{}", feature),
                        bin_id: bin,
                        bin_count: rand::rng().random_range(0..10),
                    };

                    PostgresClient::insert_bin_counts(&pool, &record)
                        .await
                        .unwrap();
                }
            }
        }

        let binned_records = PostgresClient::get_feature_bin_proportions(
            &pool,
            &ServiceInfo {
                space: SPACE.to_string(),
                name: NAME.to_string(),
                version: VERSION.to_string(),
            },
            &timestamp,
            &["feature0".to_string()],
        )
        .await
        .unwrap();

        // assert binned_records.features["test"]["decile_1"] is around .5
        let bin_proportion = binned_records
            .features
            .get("feature0")
            .unwrap()
            .get(&1)
            .unwrap();

        assert!(*bin_proportion > 0.1 && *bin_proportion < 0.2);

        let binned_records = PostgresClient::get_binned_psi_drift_records(
            &pool,
            &DriftRequest {
                space: SPACE.to_string(),
                name: NAME.to_string(),
                version: VERSION.to_string(),
                time_interval: TimeInterval::OneHour,
                max_data_points: 1000,
                drift_type: DriftType::Psi,
                ..Default::default()
            },
            &DatabaseSettings::default().retention_period,
            &ObjectStorageSettings::default(),
        )
        .await
        .unwrap();
        //
        assert_eq!(binned_records.len(), 3);
    }

    #[tokio::test]
    async fn test_postgres_cru_custom_metric() {
        let pool = db_pool().await;

        let timestamp = Utc::now();

        for i in 0..2 {
            for _ in 0..25 {
                let record = CustomMetricServerRecord {
                    created_at: Utc::now(),
                    space: SPACE.to_string(),
                    name: NAME.to_string(),
                    version: VERSION.to_string(),
                    metric: format!("metric{}", i),
                    value: rand::rng().random_range(0..10) as f64,
                };

                let result = PostgresClient::insert_custom_metric_value(&pool, &record)
                    .await
                    .unwrap();
                assert_eq!(result.rows_affected(), 1);
            }
        }

        // insert random record to test has statistics funcs handle single record
        let record = CustomMetricServerRecord {
            created_at: Utc::now(),
            space: SPACE.to_string(),
            name: NAME.to_string(),
            version: VERSION.to_string(),
            metric: "metric3".to_string(),
            value: rand::rng().random_range(0..10) as f64,
        };

        let result = PostgresClient::insert_custom_metric_value(&pool, &record)
            .await
            .unwrap();
        assert_eq!(result.rows_affected(), 1);

        let metrics = PostgresClient::get_custom_metric_values(
            &pool,
            &ServiceInfo {
                space: SPACE.to_string(),
                name: NAME.to_string(),
                version: VERSION.to_string(),
            },
            &timestamp,
            &["metric1".to_string()],
        )
        .await
        .unwrap();

        assert_eq!(metrics.len(), 1);

        let binned_records = PostgresClient::get_binned_custom_drift_records(
            &pool,
            &DriftRequest {
                space: SPACE.to_string(),
                name: NAME.to_string(),
                version: VERSION.to_string(),
                time_interval: TimeInterval::OneHour,
                max_data_points: 1000,
                drift_type: DriftType::Custom,
                ..Default::default()
            },
            &DatabaseSettings::default().retention_period,
            &ObjectStorageSettings::default(),
        )
        .await
        .unwrap();
        //
        assert_eq!(binned_records.metrics.len(), 3);
    }

    #[tokio::test]
    async fn test_postgres_user() {
        let pool = db_pool().await;

        // Create
        let user = User::new(
            "user".to_string(),
            "pass".to_string(),
            None,
            None,
            Some("admin".to_string()),
        );
        PostgresClient::insert_user(&pool, &user).await.unwrap();

        // Read
        let mut user = PostgresClient::get_user(&pool, "user")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(user.username, "user");

        // update user
        user.active = false;
        user.refresh_token = Some("token".to_string());

        // Update
        PostgresClient::update_user(&pool, &user).await.unwrap();
        let user = PostgresClient::get_user(&pool, "user")
            .await
            .unwrap()
            .unwrap();
        assert!(!user.active);
        assert_eq!(user.refresh_token.unwrap(), "token");

        // get users
        let users = PostgresClient::get_users(&pool).await.unwrap();
        assert_eq!(users.len(), 1);

        // get last admin
        let is_last_admin = PostgresClient::is_last_admin(&pool, "user").await.unwrap();
        assert!(is_last_admin);

        // delete
        PostgresClient::delete_user(&pool, "user").await.unwrap();
    }
}
