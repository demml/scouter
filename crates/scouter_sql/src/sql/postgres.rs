use crate::sql::cache::entity_cache;
use crate::sql::cache::init_entity_cache;
use crate::sql::error::SqlError;
use crate::sql::traits::{
    AlertSqlLogic, ArchiveSqlLogic, CustomMetricSqlLogic, LLMDriftSqlLogic, ObservabilitySqlLogic,
    ProfileSqlLogic, PsiSqlLogic, SpcSqlLogic, TagSqlLogic, TraceSqlLogic, UserSqlLogic,
};
use scouter_settings::DatabaseSettings;
use scouter_types::{RecordType, ServerRecords, TagRecord, ToDriftRecords, TraceServerRecord};
use sqlx::ConnectOptions;
use sqlx::{postgres::PgConnectOptions, Pool, Postgres};
use std::result::Result::Ok;
use tokio::try_join;
use tracing::log::LevelFilter;
use tracing::{debug, error, info, instrument};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PostgresClient {}

impl SpcSqlLogic for PostgresClient {}
impl CustomMetricSqlLogic for PostgresClient {}
impl PsiSqlLogic for PostgresClient {}
impl LLMDriftSqlLogic for PostgresClient {}
impl UserSqlLogic for PostgresClient {}
impl ProfileSqlLogic for PostgresClient {}
impl ObservabilitySqlLogic for PostgresClient {}
impl AlertSqlLogic for PostgresClient {}
impl ArchiveSqlLogic for PostgresClient {}
impl TraceSqlLogic for PostgresClient {}
impl TagSqlLogic for PostgresClient {}

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
        let mut opts: PgConnectOptions = database_settings.connection_uri.parse()?;

        // Sqlx logs a lot of debug information by default, which can be overwhelming.
        opts = opts.log_statements(LevelFilter::Off);

        let pool = match sqlx::postgres::PgPoolOptions::new()
            .max_connections(database_settings.max_connections)
            .connect_with(opts)
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

        // setup entity cache
        init_entity_cache(1000);

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
            .map_err(SqlError::MigrateError)?;

        debug!("Migrations complete");

        Ok(())
    }
}

pub struct MessageHandler {}

impl MessageHandler {
    const DEFAULT_BATCH_SIZE: usize = 500;
    #[instrument(skip_all)]
    pub async fn insert_server_records(
        pool: &Pool<Postgres>,
        records: &ServerRecords,
    ) -> Result<(), SqlError> {
        info!("Inserting server records: {:?}", records.record_type()?);

        let entity_id = entity_cache()
            .get_entity_id_from_uid(pool, records.uid()?)
            .await?;

        match records.record_type()? {
            RecordType::Spc => {
                let spc_records = records.to_spc_drift_records()?;
                debug!("SPC record count: {}", spc_records.len());

                for chunk in spc_records.chunks(Self::DEFAULT_BATCH_SIZE) {
                    PostgresClient::insert_spc_drift_records_batch(pool, chunk, &entity_id)
                        .await
                        .map_err(|e| {
                            error!("Failed to insert SPC drift records batch: {:?}", e);
                            e
                        })?;
                }
            }

            RecordType::Psi => {
                let psi_records = records.to_psi_drift_records()?;
                debug!("PSI record count: {}", psi_records.len());

                for chunk in psi_records.chunks(Self::DEFAULT_BATCH_SIZE) {
                    PostgresClient::insert_bin_counts_batch(pool, chunk, &entity_id)
                        .await
                        .map_err(|e| {
                            error!("Failed to insert PSI drift records batch: {:?}", e);
                            e
                        })?;
                }
            }
            RecordType::Custom => {
                let custom_records = records.to_custom_metric_drift_records()?;
                debug!("Custom record count: {}", custom_records.len());

                for chunk in custom_records.chunks(Self::DEFAULT_BATCH_SIZE) {
                    PostgresClient::insert_custom_metric_values_batch(pool, chunk, &entity_id)
                        .await
                        .map_err(|e| {
                            error!("Failed to insert custom metric records batch: {:?}", e);
                            e
                        })?;
                }
            }

            RecordType::LLMDrift => {
                debug!("LLM Drift record count: {:?}", records.len());
                let records = records.to_llm_drift_records()?;
                for record in records.iter() {
                    let _ = PostgresClient::insert_llm_drift_record(pool, record, &entity_id)
                        .await
                        .map_err(|e| {
                            error!("Failed to insert LLM drift record: {:?}", e);
                        });
                }
            }

            RecordType::LLMMetric => {
                debug!("LLM Metric record count: {:?}", records.len());
                let llm_metric_records = records.to_llm_metric_records()?;

                for chunk in llm_metric_records.chunks(Self::DEFAULT_BATCH_SIZE) {
                    PostgresClient::insert_llm_metric_values_batch(pool, chunk, &entity_id)
                        .await
                        .map_err(|e| {
                            error!("Failed to insert LLM metric records batch: {:?}", e);
                            e
                        })?;
                }
            }

            _ => {
                error!(
                    "Unsupported record type for batch insert: {:?}",
                    records.record_type()?
                );
                return Err(SqlError::UnsupportedBatchTypeError);
            }
        }

        Ok(())
    }

    pub async fn insert_trace_server_record(
        pool: &Pool<Postgres>,
        records: &TraceServerRecord,
    ) -> Result<(), SqlError> {
        let (span_batch, baggage_batch, tag_records) = records.to_records()?;

        let (span_result, baggage_result, tag_result) = try_join!(
            PostgresClient::insert_span_batch(pool, &span_batch),
            PostgresClient::insert_trace_baggage_batch(pool, &baggage_batch),
            PostgresClient::insert_tag_batch(pool, &tag_records),
        )?;

        debug!(
            span_rows = span_result.rows_affected(),
            baggage_rows = baggage_result.rows_affected(),
            total_spans = span_batch.len(),
            total_baggage = baggage_batch.len(),
            tag_rows = tag_result.rows_affected(),
            "Successfully inserted trace server records"
        );
        Ok(())
    }

    pub async fn insert_tag_record(
        pool: &Pool<Postgres>,
        record: &TagRecord,
    ) -> Result<(), SqlError> {
        let result = PostgresClient::insert_tag_batch(pool, std::slice::from_ref(record)).await?;

        debug!(
            rows_affected = result.rows_affected(),
            entity_type = record.entity_type.as_str(),
            entity_id = record.entity_id.as_str(),
            key = record.key.as_str(),
            "Successfully inserted tag record"
        );

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
    use crate::sql::traits::EntitySqlLogic;
    use chrono::{Duration, Utc};
    use potato_head::{create_score_prompt, create_uuid7};
    use rand::Rng;
    use scouter_semver::VersionType;
    use scouter_settings::ObjectStorageSettings;
    use scouter_types::llm::PaginationRequest;
    use scouter_types::psi::{Bin, BinType, PsiDriftConfig, PsiFeatureDriftProfile};
    use scouter_types::spc::SpcDriftProfile;
    use scouter_types::sql::TraceFilters;
    use scouter_types::*;
    use serde_json::Value;
    use std::collections::BTreeMap;

    const SPACE: &str = "space";
    const NAME: &str = "name";
    const VERSION: &str = "1.0.0";
    const SCOPE: &str = "scope";
    const UID: &str = "test";
    const ENTITY_ID: i32 = 9999;

    fn random_trace_record() -> TraceRecord {
        let mut rng = rand::rng();
        let random_num = rng.random_range(0..1000);
        let trace_id: String = (0..32)
            .map(|_| format!("{:x}", rng.random_range(0..16)))
            .collect();
        let span_id: String = (0..16)
            .map(|_| format!("{:x}", rng.random_range(0..16)))
            .collect();
        let created_at = Utc::now() + chrono::Duration::milliseconds(random_num);

        TraceRecord {
            trace_id: trace_id.clone(),
            created_at,
            service_name: format!("service_{}", random_num % 10),
            scope: SCOPE.to_string(),
            trace_state: "running".to_string(),
            start_time: created_at,
            end_time: created_at + chrono::Duration::milliseconds(150),
            duration_ms: 150,
            status_code: 0,
            span_count: 1,
            status_message: "OK".to_string(),
            root_span_id: span_id.clone(),
            tags: vec![],
            process_attributes: vec![],
        }
    }

    fn random_span_record(
        trace_id: &str,
        parent_span_id: Option<&str>,
        service_name: &str,
    ) -> TraceSpanRecord {
        let mut rng = rand::rng();
        let span_id: String = (0..16)
            .map(|_| format!("{:x}", rng.random_range(0..16)))
            .collect();

        let random_offset_ms = rng.random_range(0..1000);
        let duration_ms_val = rng.random_range(50..500);

        let created_at = Utc::now() + chrono::Duration::milliseconds(random_offset_ms);
        let start_time = created_at;
        let end_time = start_time + chrono::Duration::milliseconds(duration_ms_val);

        // --- Status and Kind ---
        let status_code = if rng.random_bool(0.95) { 0 } else { 2 };
        let span_kind_options = ["SERVER", "CLIENT", "INTERNAL", "PRODUCER", "CONSUMER"];
        let span_kind = span_kind_options[rng.random_range(0..span_kind_options.len())].to_string();

        TraceSpanRecord {
            created_at,
            span_id,
            trace_id: trace_id.to_string(),
            parent_span_id: parent_span_id.map(|s| s.to_string()),
            service_name: service_name.to_string(),
            scope: SCOPE.to_string(),
            span_name: format!("{}_{}", "random_operation", rng.random_range(0..10)),
            span_kind,
            start_time,
            end_time,
            duration_ms: duration_ms_val,
            status_code,
            status_message: if status_code == 2 {
                "Internal Server Error".to_string()
            } else {
                "OK".to_string()
            },
            attributes: vec![Attribute::default()],
            events: vec![],
            links: vec![],
            label: None,
            input: Value::default(),
            output: Value::default(),
            resource_attributes: vec![],
        }
    }

    pub async fn cleanup(pool: &Pool<Postgres>) {
        sqlx::raw_sql(
            r#"
            DELETE
            FROM scouter.service_entities;

            DELETE
            FROM scouter.drift_entities;

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

            DELETE
            FROM scouter.user;

            DELETE
            FROM scouter.llm_drift_record;

            DELETE
            FROM scouter.llm_drift;

            DELETE
            FROM scouter.spans;

            DELETE
            FROM scouter.trace_baggage;

            DELETE
            FROM scouter.tags;
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

    pub async fn insert_profile_to_db(
        pool: &Pool<Postgres>,
        profile: &DriftProfile,
        active: bool,
        deactivate_others: bool,
    ) -> String {
        let base_args = profile.get_base_args();
        let version_request = VersionRequest {
            version: None,
            version_type: VersionType::Minor,
            pre_tag: None,
            build_tag: None,
        };
        let version = PostgresClient::get_next_profile_version(pool, &base_args, version_request)
            .await
            .unwrap();

        let result = PostgresClient::insert_drift_profile(
            pool,
            profile,
            &base_args,
            &version,
            &active,
            &deactivate_others,
        )
        .await
        .unwrap();

        result
    }

    #[tokio::test]
    async fn test_postgres_start() {
        let _pool = db_pool().await;
    }

    #[tokio::test]
    async fn test_postgres_drift_alert() {
        let pool = db_pool().await;

        let timestamp = Utc::now();
        let entity_id = 9999;

        for _ in 0..10 {
            let alert = (0..10)
                .map(|i| (i.to_string(), i.to_string()))
                .collect::<BTreeMap<String, String>>();

            let result = PostgresClient::insert_drift_alert(&pool, &entity_id, "test", &alert)
                .await
                .unwrap();

            assert_eq!(result.rows_affected(), 1);
        }

        // get alerts
        let alert_request = DriftAlertRequest {
            uid: UID.to_string(),
            active: Some(true),
            limit: None,
            limit_datetime: None,
        };

        let alerts = PostgresClient::get_drift_alerts(&pool, &alert_request, &entity_id)
            .await
            .unwrap();
        assert!(alerts.len() > 5);

        // get alerts limit 1
        let alert_request = DriftAlertRequest {
            uid: UID.to_string(),
            active: Some(true),
            limit: Some(1),
            limit_datetime: None,
        };

        let alerts = PostgresClient::get_drift_alerts(&pool, &alert_request, &entity_id)
            .await
            .unwrap();
        assert_eq!(alerts.len(), 1);

        // get alerts limit timestamp
        let alert_request = DriftAlertRequest {
            uid: UID.to_string(),
            active: Some(true),
            limit: None,
            limit_datetime: Some(timestamp),
        };

        let alerts = PostgresClient::get_drift_alerts(&pool, &alert_request, &entity_id)
            .await
            .unwrap();
        assert!(alerts.len() > 5);
    }

    #[tokio::test]
    async fn test_postgres_spc_drift_record() {
        let pool = db_pool().await;

        let record1 = SpcRecord {
            created_at: Utc::now(),
            uid: UID.to_string(),
            feature: "test".to_string(),
            value: 1.0,
        };

        let record2 = SpcRecord {
            created_at: Utc::now(),
            uid: UID.to_string(),
            feature: "test2".to_string(),
            value: 2.0,
        };

        let result =
            PostgresClient::insert_spc_drift_records_batch(&pool, &[record1, record2], &ENTITY_ID)
                .await
                .unwrap();

        assert_eq!(result.rows_affected(), 2);
    }

    #[tokio::test]
    async fn test_postgres_bin_count() {
        let pool = db_pool().await;

        let record1 = PsiRecord {
            created_at: Utc::now(),
            uid: UID.to_string(),
            feature: "test".to_string(),
            bin_id: 1,
            bin_count: 1,
        };

        let record2 = PsiRecord {
            created_at: Utc::now(),
            uid: UID.to_string(),
            feature: "test2".to_string(),
            bin_id: 2,
            bin_count: 2,
        };

        let result =
            PostgresClient::insert_bin_counts_batch(&pool, &[record1, record2], &ENTITY_ID)
                .await
                .unwrap();

        assert_eq!(result.rows_affected(), 2);
    }

    #[tokio::test]
    async fn test_postgres_observability_record() {
        let pool = db_pool().await;

        let record = ObservabilityMetrics::default();

        let result = PostgresClient::insert_observability_record(&pool, &record, &ENTITY_ID)
            .await
            .unwrap();

        assert_eq!(result.rows_affected(), 1);
    }

    #[tokio::test]
    async fn test_postgres_crud_drift_profile() {
        let pool = db_pool().await;

        let mut spc_profile = SpcDriftProfile::default();
        let profile = DriftProfile::Spc(spc_profile.clone());
        let uid = insert_profile_to_db(&pool, &profile, false, false).await;
        assert!(!uid.is_empty());

        let entity_id = PostgresClient::get_entity_id_from_uid(&pool, &uid)
            .await
            .unwrap();

        spc_profile.scouter_version = "test".to_string();

        let result = PostgresClient::update_drift_profile(
            &pool,
            &DriftProfile::Spc(spc_profile.clone()),
            &entity_id,
        )
        .await
        .unwrap();

        assert_eq!(result.rows_affected(), 1);

        let profile = PostgresClient::get_drift_profile(&pool, &entity_id)
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
                deactivate_others: false,
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
            let mut records = Vec::new();
            for j in 0..10 {
                let record = SpcRecord {
                    created_at: Utc::now() + chrono::Duration::microseconds(j as i64),
                    uid: UID.to_string(),
                    feature: format!("test{j}"),
                    value: j as f64,
                };

                records.push(record);
            }

            let result =
                PostgresClient::insert_spc_drift_records_batch(&pool, &records, &ENTITY_ID)
                    .await
                    .unwrap();
            assert_eq!(result.rows_affected(), records.len() as u64);
        }

        let features = PostgresClient::get_spc_features(&pool, &ENTITY_ID)
            .await
            .unwrap();
        assert_eq!(features.len(), 10);

        let records =
            PostgresClient::get_spc_drift_records(&pool, &timestamp, &features, &ENTITY_ID)
                .await
                .unwrap();

        assert_eq!(records.features.len(), 10);

        let binned_records = PostgresClient::get_binned_spc_drift_records(
            &pool,
            &DriftRequest {
                uid: UID.to_string(),
                time_interval: TimeInterval::FiveMinutes,
                max_data_points: 10,
                ..Default::default()
            },
            &DatabaseSettings::default().retention_period,
            &ObjectStorageSettings::default(),
            &ENTITY_ID,
        )
        .await
        .unwrap();

        assert_eq!(binned_records.features.len(), 10);
    }

    #[tokio::test]
    async fn test_postgres_bin_proportions() {
        let pool = db_pool().await;

        let timestamp = Utc::now();

        let num_features = 3;
        let num_bins = 5;

        let features = (0..=num_features)
            .map(|feature| {
                let bins = (0..=num_bins)
                    .map(|bind_id| Bin {
                        id: bind_id,
                        lower_limit: None,
                        upper_limit: None,
                        proportion: 0.0,
                    })
                    .collect();
                let feature_name = format!("feature{feature}");
                let feature_profile = PsiFeatureDriftProfile {
                    id: feature_name.clone(),
                    bins,
                    timestamp,
                    bin_type: BinType::Numeric,
                };
                (feature_name, feature_profile)
            })
            .collect();

        let profile = &DriftProfile::Psi(psi::PsiDriftProfile::new(
            features,
            PsiDriftConfig {
                space: SPACE.to_string(),
                name: NAME.to_string(),
                version: VERSION.to_string(),
                ..Default::default()
            },
        ));
        let uid = insert_profile_to_db(&pool, profile, false, false).await;
        let entity_id = PostgresClient::get_entity_id_from_uid(&pool, &uid)
            .await
            .unwrap();

        for feature in 0..num_features {
            for bin in 0..=num_bins {
                let mut records = Vec::new();
                for j in 0..=100 {
                    let record = PsiRecord {
                        created_at: Utc::now() + chrono::Duration::microseconds(j as i64),
                        uid: uid.to_string(),
                        feature: format!("feature{feature}"),
                        bin_id: bin,
                        bin_count: rand::rng().random_range(0..10),
                    };

                    records.push(record);
                }
                PostgresClient::insert_bin_counts_batch(&pool, &records, &entity_id)
                    .await
                    .unwrap();
            }
        }

        let binned_records = PostgresClient::get_feature_distributions(
            &pool,
            &timestamp,
            &["feature0".to_string()],
            &entity_id,
        )
        .await
        .unwrap();

        // assert binned_records.features["test"]["decile_1"] is around .5
        let bin_proportion = binned_records
            .distributions
            .get("feature0")
            .unwrap()
            .bins
            .get(&1)
            .unwrap();

        assert!(*bin_proportion > 0.1 && *bin_proportion < 0.2);

        let binned_records = PostgresClient::get_binned_psi_drift_records(
            &pool,
            &DriftRequest {
                uid: UID.to_string(),
                time_interval: TimeInterval::OneHour,
                max_data_points: 1000,
                ..Default::default()
            },
            &DatabaseSettings::default().retention_period,
            &ObjectStorageSettings::default(),
            &entity_id,
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

        let (uid, entity_id) = PostgresClient::create_entity(
            &pool,
            SPACE,
            NAME,
            VERSION,
            DriftType::Custom.to_string(),
        )
        .await
        .unwrap();

        for i in 0..2 {
            let mut records = Vec::new();
            for j in 0..25 {
                let record = CustomMetricRecord {
                    created_at: Utc::now() + chrono::Duration::microseconds(j as i64),
                    uid: uid.clone(),
                    metric: format!("metric{i}"),
                    value: rand::rng().random_range(0..10) as f64,
                };
                records.push(record);
            }
            let result =
                PostgresClient::insert_custom_metric_values_batch(&pool, &records, &entity_id)
                    .await
                    .unwrap();
            assert_eq!(result.rows_affected(), 25);
        }

        // insert random record to test has statistics funcs handle single record
        let record = CustomMetricRecord {
            created_at: Utc::now(),
            uid: uid.clone(),
            metric: "metric3".to_string(),
            value: rand::rng().random_range(0..10) as f64,
        };

        let result =
            PostgresClient::insert_custom_metric_values_batch(&pool, &[record], &entity_id)
                .await
                .unwrap();
        assert_eq!(result.rows_affected(), 1);

        let metrics = PostgresClient::get_custom_metric_values(
            &pool,
            &timestamp,
            &["metric1".to_string()],
            &entity_id,
        )
        .await
        .unwrap();

        assert_eq!(metrics.len(), 1);

        let binned_records = PostgresClient::get_binned_custom_drift_records(
            &pool,
            &DriftRequest {
                uid: uid.clone(),
                time_interval: TimeInterval::OneHour,
                max_data_points: 1000,
                ..Default::default()
            },
            &DatabaseSettings::default().retention_period,
            &ObjectStorageSettings::default(),
            &entity_id,
        )
        .await
        .unwrap();
        //
        assert_eq!(binned_records.metrics.len(), 3);
    }

    #[tokio::test]
    async fn test_postgres_user() {
        let pool = db_pool().await;
        let recovery_codes = vec!["recovery_code_1".to_string(), "recovery_code_2".to_string()];

        // Create
        let user = User::new(
            "user".to_string(),
            "pass".to_string(),
            "email".to_string(),
            recovery_codes,
            None,
            None,
            None,
            None,
        );
        PostgresClient::insert_user(&pool, &user).await.unwrap();

        // Read
        let mut user = PostgresClient::get_user(&pool, "user")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(user.username, "user");
        assert_eq!(user.group_permissions, vec!["user"]);
        assert_eq!(user.email, "email");

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
        assert!(!is_last_admin);

        // delete
        PostgresClient::delete_user(&pool, "user").await.unwrap();
    }

    #[tokio::test]
    async fn test_postgres_llm_drift_record_insert_get() {
        let pool = db_pool().await;

        let (uid, entity_id) =
            PostgresClient::create_entity(&pool, SPACE, NAME, VERSION, DriftType::LLM.to_string())
                .await
                .unwrap();

        let input = "This is a test input";
        let output = "This is a test response";
        let prompt = create_score_prompt(None);

        for j in 0..10 {
            let context = serde_json::json!({
                "input": input,
                "response": output,
            });
            let record = LLMDriftRecord {
                created_at: Utc::now() + chrono::Duration::microseconds(j as i64),
                prompt: Some(prompt.model_dump_value()),
                context,
                status: Status::Pending,
                id: 0, // This will be set by the database
                uid: format!("test_{}", j),
                updated_at: None,
                score: Value::Null,
                processing_started_at: None,
                processing_ended_at: None,
                processing_duration: None,
                entity_uid: uid.clone(),
            };

            let result = PostgresClient::insert_llm_drift_record(&pool, &record, &entity_id)
                .await
                .unwrap();

            assert_eq!(result.rows_affected(), 1);
        }

        let features = PostgresClient::get_llm_drift_records(&pool, None, None, &entity_id)
            .await
            .unwrap();
        assert_eq!(features.len(), 10);

        // get pending task
        let pending_tasks = PostgresClient::get_pending_llm_drift_record(&pool)
            .await
            .unwrap();

        // assert not empty
        assert!(pending_tasks.is_some());

        // get pending task with space, name, version
        let task_input = &pending_tasks.as_ref().unwrap().context["input"];
        assert_eq!(*task_input, "This is a test input".to_string());

        // update pending task
        PostgresClient::update_llm_drift_record_status(
            &pool,
            &pending_tasks.unwrap(),
            Status::Processed,
            Some(1),
        )
        .await
        .unwrap();

        // query processed tasks
        let processed_tasks =
            PostgresClient::get_llm_drift_records(&pool, None, Some(Status::Processed), &entity_id)
                .await
                .unwrap();

        // assert not empty
        assert_eq!(processed_tasks.len(), 1);
    }

    #[tokio::test]
    async fn test_postgres_llm_drift_record_pagination() {
        let pool = db_pool().await;

        let (uid, entity_id) =
            PostgresClient::create_entity(&pool, SPACE, NAME, VERSION, DriftType::LLM.to_string())
                .await
                .unwrap();

        let input = "This is a test input";
        let output = "This is a test response";
        let prompt = create_score_prompt(None);

        for j in 0..10 {
            let context = serde_json::json!({
                "input": input,
                "response": output,
            });
            let record = LLMDriftRecord {
                created_at: Utc::now() + chrono::Duration::microseconds(j as i64),
                prompt: Some(prompt.model_dump_value()),
                context,
                score: Value::Null,
                status: Status::Pending,
                id: 0, // This will be set by the database
                uid: format!("test_{}", j),
                updated_at: None,
                processing_started_at: None,
                processing_ended_at: None,
                processing_duration: None,
                entity_uid: uid.clone(),
            };

            let result = PostgresClient::insert_llm_drift_record(&pool, &record, &entity_id)
                .await
                .unwrap();

            assert_eq!(result.rows_affected(), 1);
        }

        // Get paginated records (1st page)
        let pagination = PaginationRequest {
            limit: 5,
            cursor: None, // Start from the beginning
        };

        let paginated_features =
            PostgresClient::get_llm_drift_records_pagination(&pool, &entity_id, None, pagination)
                .await
                .unwrap();

        assert_eq!(paginated_features.items.len(), 5);
        assert!(paginated_features.next_cursor.is_some());

        // get id of the most recent record in the first page
        let last_record = paginated_features.items.first().unwrap();

        // Get paginated records (2nd page)
        let next_cursor = paginated_features.next_cursor.unwrap();
        let pagination = PaginationRequest {
            limit: 5,
            cursor: Some(next_cursor),
        };

        let paginated_features =
            PostgresClient::get_llm_drift_records_pagination(&pool, &entity_id, None, pagination)
                .await
                .unwrap();

        assert_eq!(paginated_features.items.len(), 5);
        assert!(paginated_features.next_cursor.is_none());

        // get last record of the second page
        let first_record = paginated_features.items.last().unwrap();

        let diff = last_record.id - first_record.id + 1; // +1 because IDs are inclusive
        assert!(diff == 10);
    }

    #[tokio::test]
    async fn test_postgres_llm_metrics_insert_get() {
        let pool = db_pool().await;

        let timestamp = Utc::now();

        let (uid, entity_id) =
            PostgresClient::create_entity(&pool, SPACE, NAME, VERSION, DriftType::LLM.to_string())
                .await
                .unwrap();

        for i in 0..2 {
            let mut records = Vec::new();
            for j in 0..25 {
                let record = LLMMetricRecord {
                    uid: format!("uid{i}{j}"),
                    created_at: Utc::now() + chrono::Duration::microseconds(j as i64),
                    entity_uid: uid.clone(),
                    metric: format!("metric{i}"),
                    value: rand::rng().random_range(0..10) as f64,
                };
                records.push(record);
            }
            let result =
                PostgresClient::insert_llm_metric_values_batch(&pool, &records, &entity_id)
                    .await
                    .unwrap();
            assert_eq!(result.rows_affected(), 25);
        }

        let metrics = PostgresClient::get_llm_metric_values(
            &pool,
            &timestamp,
            &["metric1".to_string()],
            &entity_id,
        )
        .await
        .unwrap();

        assert_eq!(metrics.len(), 1);
        let binned_records = PostgresClient::get_binned_llm_metric_values(
            &pool,
            &DriftRequest {
                uid: uid.clone(),
                time_interval: TimeInterval::OneHour,
                max_data_points: 1000,
                ..Default::default()
            },
            &DatabaseSettings::default().retention_period,
            &ObjectStorageSettings::default(),
            &entity_id,
        )
        .await
        .unwrap();
        //
        assert_eq!(binned_records.metrics.len(), 2);
    }

    #[tokio::test]
    async fn test_postgres_tracing_metrics() {
        let pool = db_pool().await;
        let script = std::fs::read_to_string("src/tests/script/populate_trace.sql").unwrap();
        sqlx::query(&script).execute(&pool).await.unwrap();
        let mut filters = TraceFilters::default();

        let first_batch = PostgresClient::get_traces_paginated(&pool, filters.clone())
            .await
            .unwrap();

        assert_eq!(
            first_batch.items.len(),
            50,
            "First batch should have 50 records"
        );

        // test pagination (get last record created_at and trace_id)
        let last_record = first_batch.next_cursor.unwrap();
        filters = filters.next_page(&last_record);

        let next_batch = PostgresClient::get_traces_paginated(&pool, filters.clone())
            .await
            .unwrap();

        // should be another 50 records
        assert_eq!(
            next_batch.items.len(),
            50,
            "Next batch should have 50 records"
        );

        // assert next_batch first record timestamp is <= first_batch last record timestamp
        let next_first_record = next_batch.items.first().unwrap();
        assert!(
            next_first_record.start_time <= last_record.start_time,
            "Next batch first record timestamp is not less than or equal to last record timestamp"
        );

        // test pagination for previous
        filters = filters.previous_page(&next_batch.previous_cursor.unwrap());
        let previous_batch = PostgresClient::get_traces_paginated(&pool, filters.clone())
            .await
            .unwrap();
        assert_eq!(
            previous_batch.items.len(),
            50,
            "Previous batch should have 50 records"
        );

        // Filter records to find item with >5 spans
        let filtered_record = first_batch
            .items
            .iter()
            .find(|record| record.span_count > 5)
            .unwrap();

        filters.cursor_start_time = None;
        filters.cursor_trace_id = None;

        let records = PostgresClient::get_traces_paginated(&pool, filters.clone())
            .await
            .unwrap();

        // Records are randomly generated, so just assert we get some records back
        assert!(
            !records.items.is_empty(),
            "Should return records with specified filters"
        );

        // get spans for filtered trace
        let spans = PostgresClient::get_trace_spans(
            &pool,
            &filtered_record.trace_id,
            Some(&filtered_record.service_name),
        )
        .await
        .unwrap();

        assert!(spans.len() == filtered_record.span_count as usize);

        let start_time = filtered_record.start_time - chrono::Duration::hours(48);
        let end_time = filtered_record.start_time + chrono::Duration::minutes(5);

        // make request for trace metrics
        let trace_metrics =
            PostgresClient::get_trace_metrics(&pool, None, start_time, end_time, "5 minutes")
                .await
                .unwrap();

        // assert we have data points
        assert!(trace_metrics.len() >= 10);

        // get paginated traces with tags
        let filters = scouter_types::sql::TraceFilters {
            tags: Some(vec![("span.tag.host=host-4".to_string())]),
            ..Default::default()
        };
        let tagged_batch = PostgresClient::get_traces_paginated(&pool, filters)
            .await
            .unwrap();
        assert!(!tagged_batch.items.is_empty());
    }

    #[tokio::test]
    async fn test_postgres_tracing_insert() {
        let pool = db_pool().await;

        // create parent trace
        let mut trace_record = random_trace_record();
        let trace_id = trace_record.trace_id.clone();

        // create spans
        let root_span = random_span_record(&trace_id, None, &trace_record.service_name);
        let child_span =
            random_span_record(&trace_id, Some(&root_span.span_id), &root_span.service_name);

        // set root span id in trace record
        trace_record.root_span_id = root_span.span_id.clone();

        // insert spans
        let result =
            PostgresClient::insert_span_batch(&pool, &[root_span.clone(), child_span.clone()])
                .await
                .unwrap();

        assert_eq!(result.rows_affected(), 2);

        let inserted_created_at = trace_record.created_at;
        let inserted_trace_id = trace_record.trace_id.clone();

        let trace_filter = TraceFilters {
            cursor_start_time: Some(inserted_created_at + Duration::days(1)),
            cursor_trace_id: Some(inserted_trace_id),
            start_time: Some(inserted_created_at - Duration::minutes(5)),
            end_time: Some(inserted_created_at + Duration::days(1)),
            ..TraceFilters::default()
        };

        let traces = PostgresClient::get_traces_paginated(&pool, trace_filter)
            .await
            .unwrap();

        assert_eq!(traces.items.len(), 1);
        let retrieved_trace = &traces.items[0];
        // assert span count is 2
        assert_eq!(retrieved_trace.span_count, 2);

        let baggage = TraceBaggageRecord {
            created_at: Utc::now(),
            trace_id: trace_record.trace_id.clone(),
            scope: "test_scope".to_string(),
            key: "user_id".to_string(),
            value: "12345".to_string(),
        };

        let result =
            PostgresClient::insert_trace_baggage_batch(&pool, std::slice::from_ref(&baggage))
                .await
                .unwrap();

        assert_eq!(result.rows_affected(), 1);

        let retrieved_baggage =
            PostgresClient::get_trace_baggage_records(&pool, &trace_record.trace_id)
                .await
                .unwrap();

        assert_eq!(retrieved_baggage.len(), 1);
    }

    #[tokio::test]
    async fn test_postgres_tags() {
        let pool = db_pool().await;
        let uid = create_uuid7();

        let tag1 = TagRecord {
            entity_id: uid.clone(),
            entity_type: "service".to_string(),
            key: "env".to_string(),
            value: "production".to_string(),
        };

        let tag2 = TagRecord {
            entity_id: uid.clone(),
            entity_type: "service".to_string(),
            key: "team".to_string(),
            value: "backend".to_string(),
        };

        let result = PostgresClient::insert_tag_batch(&pool, &[tag1.clone(), tag2.clone()])
            .await
            .unwrap();

        assert_eq!(result.rows_affected(), 2);

        let tags = PostgresClient::get_tags(&pool, "service", &uid)
            .await
            .unwrap();

        assert_eq!(tags.len(), 2);
    }
}
