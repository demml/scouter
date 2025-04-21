use crate::sql::query::Queries;
use crate::sql::schema::{AlertWrapper, Entity, ObservabilityResult, UpdateAlertResult};
use crate::sql::traits::{CustomMetricSqlLogic, PsiSqlLogic, SpcSqlLogic};
use crate::sql::utils::pg_rows_to_server_records;
use chrono::{DateTime, Utc};
use scouter_contracts::{
    DriftAlertRequest, ObservabilityMetricRequest, ServiceInfo, UpdateAlertStatus,
};

use scouter_error::{ScouterError, SqlError, UtilError};
use scouter_settings::{DatabaseSettings, ObjectStorageSettings};
use scouter_types::DriftType;
use scouter_types::{
    alert::Alert, ObservabilityMetrics, RecordType, ServerRecords, TimeInterval, ToDriftRecords,
};

use sqlx::{
    postgres::{PgPoolOptions, PgQueryResult},
    Pool, Postgres, Transaction,
};
use std::collections::BTreeMap;
use std::result::Result::Ok;
use std::sync::Arc;
use tracing::{debug, error, info, instrument};

use super::traits::{ProfileSqlLogic, UserSqlLogic};

// TODO: Explore refactoring and breaking this out into multiple client types (i.e., spc, psi, etc.)
// Postgres client is one of the lowest-level abstractions so it may not be worth it, as it could make server logic annoying. Worth exploring though.

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PostgresClient {
    pub pool: Pool<Postgres>,
    pub retention_period: i64,
    pub storage_settings: ObjectStorageSettings,
}

impl SpcSqlLogic for PostgresClient {}
impl CustomMetricSqlLogic for PostgresClient {}
impl PsiSqlLogic for PostgresClient {}
impl UserSqlLogic for PostgresClient {}
impl ProfileSqlLogic for PostgresClient {}

impl PostgresClient {
    /// Create a new PostgresClient
    ///
    /// # Arguments
    ///
    /// * `pool` - An optional database pool
    ///
    /// # Returns
    ///
    /// * `Result<Self, SqlError>` - Result of the database pool
    pub async fn new(
        pool: Option<Pool<Postgres>>,
        database_settings: &DatabaseSettings,
        storage_settings: &ObjectStorageSettings,
    ) -> Result<Arc<Self>, SqlError> {
        let pool = pool.unwrap_or(
            Self::create_db_pool(database_settings)
                .await
                .map_err(SqlError::traced_connection_error)?,
        );
        let retention_period = database_settings.retention_period;

        let client = Self {
            pool,
            retention_period,
            storage_settings: storage_settings.clone(),
        };

        // run migrations
        client.run_migrations().await?;

        Ok(Arc::new(client))
    }

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

        Ok(pool)
    }

    async fn run_migrations(&self) -> Result<(), SqlError> {
        info!("Running migrations");
        sqlx::migrate!("src/migrations")
            .run(&self.pool)
            .await
            .map_err(|e| SqlError::MigrationError(format!("{}", e)))?;

        debug!("Migrations complete");

        Ok(())
    }

    /// Inserts a drift alert into the database
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the service to insert the alert for
    /// * `space` - The name of the space to insert the alert for
    /// * `version` - The version of the service to insert the alert for
    /// * `alert` - The alert to insert into the database
    ///
    pub async fn insert_drift_alert(
        &self,
        service_info: &ServiceInfo,
        feature: &str,
        alert: &BTreeMap<String, String>,
        drift_type: &DriftType,
    ) -> Result<PgQueryResult, SqlError> {
        let query = Queries::InsertDriftAlert.get_query();

        let query_result: std::result::Result<PgQueryResult, SqlError> = sqlx::query(&query.sql)
            .bind(&service_info.name)
            .bind(&service_info.space)
            .bind(&service_info.version)
            .bind(feature)
            .bind(serde_json::to_value(alert).unwrap())
            .bind(drift_type.to_string())
            .execute(&self.pool)
            .await
            .map_err(SqlError::traced_query_error);

        match query_result {
            Ok(result) => Ok(result),
            Err(e) => Err(e),
        }
    }

    /// Get drift alerts from the database
    ///
    /// # Arguments
    ///
    /// * `params` - The drift alert request parameters
    ///
    /// # Returns
    ///
    /// * `Result<Vec<Alert>, SqlError>` - Result of the query
    pub async fn get_drift_alerts(
        &self,
        params: &DriftAlertRequest,
    ) -> Result<Vec<Alert>, SqlError> {
        let query = Queries::GetDriftAlerts.get_query().sql;

        // check if active (status can be 'active' or  'acknowledged')
        let query = if params.active.unwrap_or(false) {
            format!("{} AND active = true", query)
        } else {
            query
        };

        let query = format!("{} ORDER BY created_at DESC", query);

        let query = if let Some(limit) = params.limit {
            format!("{} LIMIT {}", query, limit)
        } else {
            query
        };

        // convert limit timestamp to string if it exists, leave as None if not

        let result: Result<Vec<AlertWrapper>, SqlError> = sqlx::query_as(&query)
            .bind(&params.version)
            .bind(&params.name)
            .bind(&params.space)
            .bind(params.limit_datetime)
            .fetch_all(&self.pool)
            .await
            .map_err(SqlError::traced_query_error);

        result.map(|result| result.into_iter().map(|wrapper| wrapper.0).collect())
    }

    pub async fn update_drift_alert_status(
        &self,
        params: &UpdateAlertStatus,
    ) -> Result<UpdateAlertResult, SqlError> {
        let query = Queries::UpdateAlertStatus.get_query();

        let result: Result<UpdateAlertResult, SqlError> = sqlx::query_as(&query.sql)
            .bind(params.id)
            .bind(params.active)
            .fetch_one(&self.pool)
            .await
            .map_err(SqlError::traced_query_error);

        match result {
            Ok(result) => Ok(result),
            Err(e) => Err(SqlError::traced_query_error(e.to_string())),
        }
    }

    // Inserts a drift record into the database
    //
    // # Arguments
    //
    // * `record` - A drift record to insert into the database
    // * `table_name` - The name of the table to insert the record into
    //
    pub async fn insert_observability_record(
        &self,
        record: &ObservabilityMetrics,
    ) -> Result<PgQueryResult, SqlError> {
        let query = Queries::InsertObservabilityRecord.get_query();
        let route_metrics = serde_json::to_value(&record.route_metrics)
            .map_err(UtilError::traced_serialize_error)?;

        sqlx::query(&query.sql)
            .bind(&record.space)
            .bind(&record.name)
            .bind(&record.version)
            .bind(record.request_count)
            .bind(record.error_count)
            .bind(route_metrics)
            .execute(&self.pool)
            .await
            .map_err(SqlError::traced_query_error)
    }

    pub async fn get_binned_observability_metrics(
        &self,
        params: &ObservabilityMetricRequest,
    ) -> Result<Vec<ObservabilityResult>, SqlError> {
        let query = Queries::GetBinnedObservabilityMetrics.get_query();

        let time_interval = TimeInterval::from_string(&params.time_interval).to_minutes();

        let bin = time_interval as f64 / params.max_data_points as f64;

        let observability_metrics: Result<Vec<ObservabilityResult>, sqlx::Error> =
            sqlx::query_as(&query.sql)
                .bind(bin)
                .bind(time_interval)
                .bind(&params.name)
                .bind(&params.space)
                .bind(&params.version)
                .fetch_all(&self.pool)
                .await;

        observability_metrics.map_err(SqlError::traced_query_error)
    }

    /// Function to get entities for archival
    ///
    /// # Arguments
    /// * `record_type` - The type of record to get entities for
    /// * `retention_period` - The retention period to get entities for
    ///
    pub async fn get_entities_to_archive(
        &self,
        record_type: &RecordType,
    ) -> Result<Vec<Entity>, SqlError> {
        let query = match record_type {
            RecordType::Spc => Queries::GetSpcEntities.get_query(),
            RecordType::Psi => Queries::GetBinCountEntities.get_query(),
            RecordType::Custom => Queries::GetCustomEntities.get_query(),
            _ => {
                return Err(SqlError::traced_invalid_record_type_error(record_type));
            }
        };

        let entities: Vec<Entity> = sqlx::query_as(&query.sql)
            .bind(&self.retention_period)
            .fetch_all(&self.pool)
            .await
            .map_err(SqlError::traced_get_entities_error)?;

        Ok(entities)
    }

    /// Function to get data for archival
    ///
    /// # Arguments
    /// * `record_type` - The type of record to get data for
    /// * `days` - The number of days to get data for
    ///
    /// # Returns
    /// * `Result<ServerRecords, SqlError>` - Result of the query
    ///
    /// # Errors
    /// * `SqlError` - If the query fails
    pub async fn get_data_to_archive(
        space: &str,
        name: &str,
        version: &str,
        begin_timestamp: &DateTime<Utc>,
        end_timestamp: &DateTime<Utc>,
        record_type: &RecordType,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<ServerRecords, SqlError> {
        let query = match record_type {
            RecordType::Spc => Queries::GetSpcDataForArchive.get_query(),
            RecordType::Psi => Queries::GetBinCountDataForArchive.get_query(),
            RecordType::Custom => Queries::GetCustomDataForArchive.get_query(),
            _ => {
                return Err(SqlError::traced_invalid_record_type_error(record_type));
            }
        };
        let rows = sqlx::query(&query.sql)
            .bind(begin_timestamp)
            .bind(end_timestamp)
            .bind(space)
            .bind(name)
            .bind(version)
            .fetch_all(&mut **tx)
            .await
            .map_err(SqlError::traced_get_entity_data_error)?;

        // need to convert the rows to server records (storage dataframe expects this)
        pg_rows_to_server_records(&rows, record_type)
    }

    pub async fn update_data_to_archived(
        space: &str,
        name: &str,
        version: &str,
        begin_timestamp: &DateTime<Utc>,
        end_timestamp: &DateTime<Utc>,
        record_type: &RecordType,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), SqlError> {
        let query = match record_type {
            RecordType::Spc => Queries::UpdateSpcEntities.get_query(),
            RecordType::Psi => Queries::UpdateBinCountEntities.get_query(),
            RecordType::Custom => Queries::UpdateCustomEntities.get_query(),
            _ => {
                return Err(SqlError::traced_invalid_record_type_error(record_type));
            }
        };
        sqlx::query(&query.sql)
            .bind(begin_timestamp)
            .bind(end_timestamp)
            .bind(space)
            .bind(name)
            .bind(version)
            .execute(&mut **tx)
            .await
            .map_err(SqlError::traced_get_entity_data_error)?;

        Ok(())
    }
}

pub enum MessageHandler {
    Postgres(Arc<PostgresClient>),
}

impl MessageHandler {
    #[instrument(skip(self, records), name = "Insert Server Records")]
    pub async fn insert_server_records(&self, records: &ServerRecords) -> Result<(), ScouterError> {
        match self {
            Self::Postgres(client) => {
                match records.record_type()? {
                    RecordType::Spc => {
                        debug!("SPC record count: {:?}", records.len());
                        let records = records.to_spc_drift_records()?;
                        for record in records.iter() {
                            let _ = client
                                .insert_spc_drift_record(&client.pool, record)
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
                            let _ = client
                                .insert_observability_record(record)
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
                            let _ = client
                                .insert_bin_counts(&client.pool, record)
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
                            let _ = client
                                .insert_custom_metric_value(&client.pool, record)
                                .await
                                .map_err(|e| {
                                    error!("Failed to insert bin count record: {:?}", e);
                                });
                        }
                    }
                };
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::sql::schema::User;
    use rand::Rng;
    use scouter_contracts::{DriftRequest, GetProfileRequest, ProfileStatusRequest};
    use scouter_types::spc::SpcDriftProfile;
    use scouter_types::*;
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

    pub async fn db_client() -> Arc<PostgresClient> {
        let client = PostgresClient::new(
            None,
            &DatabaseSettings::default(),
            &ObjectStorageSettings::default(),
        )
        .await
        .unwrap();

        cleanup(&client.pool).await;

        client
    }

    #[tokio::test]
    async fn test_postgres() {
        let _client = db_client().await;
    }

    #[tokio::test]
    async fn test_postgres_drift_alert() {
        let client = db_client().await;

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

            let result = client
                .insert_drift_alert(&service_info, "test", &alert, &DriftType::Spc)
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

        let alerts = client.get_drift_alerts(&alert_request).await.unwrap();
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

        let alerts = client.get_drift_alerts(&alert_request).await.unwrap();
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

        let alerts = client.get_drift_alerts(&alert_request).await.unwrap();
        assert!(alerts.len() > 5);
    }

    #[tokio::test]
    async fn test_postgres_spc_drift_record() {
        let client = db_client().await;

        let record = SpcServerRecord {
            created_at: Utc::now(),
            space: SPACE.to_string(),
            name: NAME.to_string(),
            version: VERSION.to_string(),
            feature: "test".to_string(),
            value: 1.0,
        };

        let result = client
            .insert_spc_drift_record(&client.pool, &record)
            .await
            .unwrap();

        assert_eq!(result.rows_affected(), 1);
    }

    #[tokio::test]
    async fn test_postgres_bin_count() {
        let client = db_client().await;

        let record = PsiServerRecord {
            created_at: Utc::now(),
            space: SPACE.to_string(),
            name: NAME.to_string(),
            version: VERSION.to_string(),
            feature: "test".to_string(),
            bin_id: 1,
            bin_count: 1,
        };

        let result = client
            .insert_bin_counts(&client.pool, &record)
            .await
            .unwrap();

        assert_eq!(result.rows_affected(), 1);
    }

    #[tokio::test]
    async fn test_postgres_observability_record() {
        let client = db_client().await;

        let record = ObservabilityMetrics::default();

        let result = client.insert_observability_record(&record).await.unwrap();

        assert_eq!(result.rows_affected(), 1);
    }

    #[tokio::test]
    async fn test_postgres_crud_drift_profile() {
        let client = PostgresClient::new(
            None,
            &DatabaseSettings::default(),
            &ObjectStorageSettings::default(),
        )
        .await
        .unwrap();
        cleanup(&client.pool).await;

        let mut spc_profile = SpcDriftProfile::default();

        let result = client
            .insert_drift_profile(&client.pool, &DriftProfile::Spc(spc_profile.clone()))
            .await
            .unwrap();

        assert_eq!(result.rows_affected(), 1);

        spc_profile.scouter_version = "test".to_string();

        let result = client
            .update_drift_profile(&client.pool, &DriftProfile::Spc(spc_profile.clone()))
            .await
            .unwrap();

        assert_eq!(result.rows_affected(), 1);

        let profile = client
            .get_drift_profile(
                &client.pool,
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

        client
            .update_drift_profile_status(
                &client.pool,
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
        let client = db_client().await;

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

                let result = client
                    .insert_spc_drift_record(&client.pool, &record)
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

        let features = client
            .get_spc_features(&client.pool, &service_info)
            .await
            .unwrap();
        assert_eq!(features.len(), 10);

        let records = client
            .get_spc_drift_records(&client.pool, &service_info, &timestamp, &features)
            .await
            .unwrap();

        assert_eq!(records.features.len(), 10);

        let binned_records = client
            .get_binned_spc_drift_records(
                &client.pool,
                &DriftRequest {
                    space: SPACE.to_string(),
                    name: NAME.to_string(),
                    version: VERSION.to_string(),
                    time_interval: TimeInterval::FiveMinutes,
                    max_data_points: 10,
                    drift_type: DriftType::Spc,
                    custom_interval: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(binned_records.features.len(), 10);
    }

    #[tokio::test]
    async fn test_postgres_bin_proportions() {
        let client = db_client().await;

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

                    client
                        .insert_bin_counts(&client.pool, &record)
                        .await
                        .unwrap();
                }
            }
        }

        let binned_records = client
            .get_feature_bin_proportions(
                &client.pool,
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

        let binned_records = client
            .get_binned_psi_drift_records(
                &client.pool,
                &DriftRequest {
                    space: SPACE.to_string(),
                    name: NAME.to_string(),
                    version: VERSION.to_string(),
                    time_interval: TimeInterval::OneHour,
                    max_data_points: 1000,
                    drift_type: DriftType::Psi,
                    custom_interval: None,
                },
                &client.retention_period,
                &client.storage_settings,
            )
            .await
            .unwrap();
        //
        assert_eq!(binned_records.len(), 3);
    }

    #[tokio::test]
    async fn test_postgres_cru_custom_metric() {
        let client = db_client().await;

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

                let result = client
                    .insert_custom_metric_value(&client.pool, &record)
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

        let result = client
            .insert_custom_metric_value(&client.pool, &record)
            .await
            .unwrap();
        assert_eq!(result.rows_affected(), 1);

        let metrics = client
            .get_custom_metric_values(
                &client.pool,
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

        let binned_records = client
            .get_binned_custom_drift_records(
                &client.pool,
                &DriftRequest {
                    space: SPACE.to_string(),
                    name: NAME.to_string(),
                    version: VERSION.to_string(),
                    time_interval: TimeInterval::OneHour,
                    max_data_points: 1000,
                    drift_type: DriftType::Custom,
                    custom_interval: None,
                },
            )
            .await
            .unwrap();
        //
        assert_eq!(binned_records.metrics.len(), 3);
    }

    #[tokio::test]
    async fn test_postgres_user() {
        let client = db_client().await;

        // Create
        let user = User::new(
            "user".to_string(),
            "pass".to_string(),
            None,
            None,
            Some("admin".to_string()),
        );
        client.insert_user(&client.pool, &user).await.unwrap();

        // Read
        let mut user = client
            .get_user(&client.pool, "user")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(user.username, "user");

        // update user
        user.active = false;
        user.refresh_token = Some("token".to_string());

        // Update
        client.update_user(&client.pool, &user).await.unwrap();
        let user = client
            .get_user(&client.pool, "user")
            .await
            .unwrap()
            .unwrap();
        assert!(!user.active);
        assert_eq!(user.refresh_token.unwrap(), "token");

        // get users
        let users = client.get_users(&client.pool).await.unwrap();
        assert_eq!(users.len(), 1);

        // get last admin
        let is_last_admin = client.is_last_admin(&client.pool, "user").await.unwrap();
        assert!(is_last_admin);

        // delete
        client.delete_user(&client.pool, "user").await.unwrap();
    }
}
