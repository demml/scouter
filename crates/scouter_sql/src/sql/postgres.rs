use crate::sql::aggregator::init_trace_cache;
use crate::sql::cache::entity_cache;
use crate::sql::cache::init_entity_cache;
use crate::sql::error::SqlError;
use crate::sql::traits::{
    AgentDriftSqlLogic, AlertSqlLogic, ArchiveSqlLogic, CustomMetricSqlLogic,
    ObservabilitySqlLogic, ProfileSqlLogic, PsiSqlLogic, SpcSqlLogic, TagSqlLogic, TraceSqlLogic,
    UserSqlLogic,
};
use scouter_settings::DatabaseSettings;
use scouter_types::{RecordType, ServerRecords, TagRecord, ToDriftRecords, TraceServerRecord};
use sqlx::ConnectOptions;
use sqlx::{postgres::PgConnectOptions, Pool, Postgres};
use std::result::Result::Ok;
use std::time::Duration;
use tokio::try_join;
use tracing::log::LevelFilter;
use tracing::{debug, error, info, instrument};
const DEFAULT_BATCH_SIZE: usize = 500;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PostgresClient {}

impl SpcSqlLogic for PostgresClient {}
impl CustomMetricSqlLogic for PostgresClient {}
impl PsiSqlLogic for PostgresClient {}
impl AgentDriftSqlLogic for PostgresClient {}
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
            .min_connections(database_settings.min_connections)
            .acquire_timeout(Duration::from_secs(
                database_settings.db_acquire_timeout_seconds,
            ))
            .idle_timeout(Duration::from_secs(
                database_settings.db_idle_timeout_seconds,
            ))
            .max_lifetime(Duration::from_secs(
                database_settings.db_max_lifetime_seconds,
            ))
            .test_before_acquire(database_settings.db_test_before_acquire)
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
        init_entity_cache(database_settings.entity_cache_size);

        // setup trace cache
        init_trace_cache(
            pool.clone(),
            database_settings.flush_interval,
            database_settings.stale_threshold,
            database_settings.max_cache_size,
        )
        .await?;

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
    #[instrument(skip_all)]
    pub async fn insert_server_records(
        pool: &Pool<Postgres>,
        records: ServerRecords,
    ) -> Result<(), SqlError> {
        debug!("Inserting server records: {:?}", records.record_type()?);

        let entity_id = entity_cache()
            .get_entity_id_from_uid(pool, records.uid()?)
            .await
            .inspect_err(|e| error!("Failed to get entity ID from UID: {:?}", e))?;

        match records.record_type()? {
            RecordType::Spc => {
                let spc_records = records.to_spc_drift_records()?;
                debug!("SPC record count: {}", spc_records.len());

                for chunk in spc_records.chunks(DEFAULT_BATCH_SIZE) {
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

                for chunk in psi_records.chunks(DEFAULT_BATCH_SIZE) {
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

                for chunk in custom_records.chunks(DEFAULT_BATCH_SIZE) {
                    PostgresClient::insert_custom_metric_values_batch(pool, chunk, &entity_id)
                        .await
                        .map_err(|e| {
                            error!("Failed to insert custom metric records batch: {:?}", e);
                            e
                        })?;
                }
            }

            RecordType::AgentEval => {
                debug!("Agent eval record count: {:?}", records.len());
                let records = records.to_agent_eval_records()?;
                for record in records {
                    let _ = PostgresClient::insert_agent_eval_record(pool, record, &entity_id)
                        .await
                        .map_err(|e| {
                            error!("Failed to insert agent eval record: {:?}", e);
                        });
                }
            }

            RecordType::AgentTask => {
                debug!("Agent task count: {:?}", records.len());
                let records = records.to_agent_task_records()?;
                for chunk in records.chunks(DEFAULT_BATCH_SIZE) {
                    PostgresClient::insert_eval_task_results_batch(pool, chunk, &entity_id)
                        .await
                        .map_err(|e| {
                            error!("Failed to insert agent task records batch: {:?}", e);
                            e
                        })?;
                }
            }

            RecordType::AgentWorkflow => {
                debug!("Agent workflow count: {:?}", records.len());
                let records = records.to_agent_workflow_records()?;
                for record in records {
                    let _ = PostgresClient::insert_agent_eval_workflow_record(
                        pool, &record, &entity_id,
                    )
                    .await
                    .map_err(|e| {
                        error!("Failed to insert agent workflow record: {:?}", e);
                    });
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
        records: TraceServerRecord,
    ) -> Result<(), SqlError> {
        let (span_batch, baggage_batch, tag_records) = records.to_records()?;
        let span_count = span_batch.len();

        // ── Update TraceCache for summary aggregation ─────────────────────────
        // TraceCache accumulates span stats in memory; flush target is TraceSummaryService.
        let trace_cache = crate::sql::aggregator::get_trace_cache().await;
        for span in &span_batch {
            trace_cache.update_trace(span).await;
        }

        // ── Write spans to Delta Lake (primary trace store) ──────────────────
        if let Some(trace_service) =
            scouter_dataframe::parquet::tracing::service::get_trace_span_service()
        {
            if let Err(e) = trace_service.write_spans(span_batch).await {
                error!("Failed to write spans to Delta Lake: {:?}", e);
            }
        } else {
            error!("TraceSpanService not initialized — spans will be lost");
        }

        // ── Baggage and tags remain in Postgres ───────────────────────────────
        let (baggage_result, tag_result) = try_join!(
            PostgresClient::insert_trace_baggage_batch(pool, &baggage_batch),
            PostgresClient::insert_tag_batch(pool, &tag_records),
        )?;

        debug!(
            span_count,
            baggage_rows = baggage_result.rows_affected(),
            total_baggage = baggage_batch.len(),
            tag_rows = tag_result.rows_affected(),
            "Successfully processed trace server records"
        );
        Ok(())
    }

    pub async fn insert_tag_record(
        pool: &Pool<Postgres>,
        record: TagRecord,
    ) -> Result<(), SqlError> {
        let result = PostgresClient::insert_tag_batch(pool, std::slice::from_ref(&record)).await?;

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
    use potato_head::create_uuid7;
    use rand::Rng;
    use scouter_semver::VersionType;
    use scouter_settings::ObjectStorageSettings;
    use scouter_types::agent::ExecutionPlan;
    use scouter_types::psi::{Bin, BinType, PsiDriftConfig, PsiFeatureDriftProfile};
    use scouter_types::spc::SpcDriftProfile;
    use scouter_types::*;
    use serde_json::Value;

    const SPACE: &str = "space";
    const NAME: &str = "name";
    const VERSION: &str = "1.0.0";
    const ENTITY_ID: i32 = 9999;

    pub async fn cleanup(pool: &Pool<Postgres>) {
        sqlx::raw_sql(
            r#"

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
            FROM scouter.agent_eval_record;

            DELETE
            FROM scouter.agent_eval_task;

            DELETE
            FROM scouter.agent_eval_workflow;

            DELETE
            FROM scouter.spans;

            DELETE
            FROM scouter.traces;

            DELETE
            FROM scouter.trace_entities;

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
        let entity_id = 9999;

        // Insert 10 alerts with slight delays to ensure different timestamps
        for i in 0..10 {
            let alert = AlertMap::Custom(custom::ComparisonMetricAlert {
                metric_name: "test".to_string(),
                baseline_value: 0 as f64,
                observed_value: i as f64,
                delta: None,
                alert_threshold: AlertThreshold::Above,
            });

            let result = PostgresClient::insert_drift_alert(&pool, &entity_id, &alert)
                .await
                .unwrap();

            assert_eq!(result.rows_affected(), 1);

            // Small delay to ensure timestamp ordering
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        // Test 1: Get first page with default limit (50)
        let request = DriftAlertPaginationRequest {
            uid: create_uuid7(),
            active: Some(true),
            limit: Some(50),
            ..Default::default()
        };

        let response = PostgresClient::get_paginated_drift_alerts(&pool, &request, &entity_id)
            .await
            .unwrap();

        assert_eq!(response.items.len(), 10);
        assert!(!response.has_next); // No more items
        assert!(!response.has_previous); // First page
        assert!(response.next_cursor.is_none());
        assert!(response.previous_cursor.is_some());

        // Test 2: Paginate with limit of 3 - forward direction
        let request = DriftAlertPaginationRequest {
            uid: create_uuid7(),
            active: Some(true),
            limit: Some(3),
            direction: Some("next".to_string()),
            ..Default::default()
        };

        let page1 = PostgresClient::get_paginated_drift_alerts(&pool, &request, &entity_id)
            .await
            .unwrap();

        assert_eq!(page1.items.len(), 3);
        assert!(page1.has_next);
        assert!(!page1.has_previous);
        assert!(page1.next_cursor.is_some());
        assert!(page1.previous_cursor.is_some());

        // Test 3: Get second page using next cursor
        let next_cursor = page1.next_cursor.unwrap();
        let request = DriftAlertPaginationRequest {
            uid: create_uuid7(),
            active: Some(true),
            limit: Some(3),
            cursor_created_at: Some(next_cursor.created_at),
            cursor_id: Some(next_cursor.id as i32),
            direction: Some("next".to_string()),
            start_datetime: None,
            end_datetime: None,
        };

        let page2 = PostgresClient::get_paginated_drift_alerts(&pool, &request, &entity_id)
            .await
            .unwrap();

        assert_eq!(page2.items.len(), 3);
        assert!(page2.has_next); // More items available
        assert!(page2.has_previous); // Can go back
        assert!(page2.next_cursor.is_some());
        assert!(page2.previous_cursor.is_some());

        // Verify no overlap between pages
        let page1_ids: std::collections::HashSet<_> = page1.items.iter().map(|a| a.id).collect();
        let page2_ids: std::collections::HashSet<_> = page2.items.iter().map(|a| a.id).collect();
        assert!(page1_ids.is_disjoint(&page2_ids));

        // Test 4: Navigate backward using previous cursor
        let prev_cursor = page2.previous_cursor.unwrap();
        let request = DriftAlertPaginationRequest {
            uid: create_uuid7(),
            active: Some(true),
            limit: Some(3),
            cursor_created_at: Some(prev_cursor.created_at),
            cursor_id: Some(prev_cursor.id as i32),
            direction: Some("previous".to_string()),
            start_datetime: None,
            end_datetime: None,
        };

        let page_back = PostgresClient::get_paginated_drift_alerts(&pool, &request, &entity_id)
            .await
            .unwrap();

        assert_eq!(page_back.items.len(), 3);
        assert!(page_back.has_next); // Can go forward
                                     // Should return to page1 items
        assert_eq!(
            page_back.items.iter().map(|a| a.id).collect::<Vec<_>>(),
            page1.items.iter().map(|a| a.id).collect::<Vec<_>>()
        );

        // Test 5: Filter by active status
        // Deactivate some alerts first
        let to_deactivate = &page1.items[0];
        let update_request = UpdateAlertStatus {
            id: to_deactivate.id,
            active: false,
            space: "test_space".to_string(),
        };

        PostgresClient::update_drift_alert_status(&pool, &update_request)
            .await
            .unwrap();

        // Query only active alerts
        let request = DriftAlertPaginationRequest {
            uid: create_uuid7(),
            active: Some(true),
            limit: Some(50),
            ..Default::default()
        };

        let active_alerts = PostgresClient::get_paginated_drift_alerts(&pool, &request, &entity_id)
            .await
            .unwrap();

        assert_eq!(active_alerts.items.len(), 9); // 10 - 1 deactivated
        assert!(active_alerts.items.iter().all(|a| a.active));

        // Query only inactive alerts
        let request = DriftAlertPaginationRequest {
            uid: create_uuid7(),
            active: Some(false),
            limit: Some(50),
            ..Default::default()
        };

        let inactive_alerts =
            PostgresClient::get_paginated_drift_alerts(&pool, &request, &entity_id)
                .await
                .unwrap();

        assert_eq!(inactive_alerts.items.len(), 1);
        assert!(!inactive_alerts.items[0].active);

        // Test 6: Query all alerts (active and inactive)
        let request = DriftAlertPaginationRequest {
            uid: create_uuid7(),
            active: None, // No filter
            limit: Some(50),
            ..Default::default()
        };

        let all_alerts = PostgresClient::get_paginated_drift_alerts(&pool, &request, &entity_id)
            .await
            .unwrap();

        assert_eq!(all_alerts.items.len(), 10);

        // Test 7: Empty result set
        let request = DriftAlertPaginationRequest {
            uid: create_uuid7(),
            active: Some(true),
            limit: Some(3),
            ..Default::default()
        };

        let non_existent = PostgresClient::get_paginated_drift_alerts(&pool, &request, &999999)
            .await
            .unwrap();

        assert_eq!(non_existent.items.len(), 0);
        assert!(!non_existent.has_next);
        assert!(!non_existent.has_previous);
        assert!(non_existent.next_cursor.is_none());
        assert!(non_existent.previous_cursor.is_none());
    }

    #[tokio::test]
    async fn test_postgres_spc_drift_record() {
        let pool = db_pool().await;

        let record1 = SpcRecord {
            created_at: Utc::now(),
            uid: create_uuid7(),
            feature: "test".to_string(),
            value: 1.0,
            entity_id: 0,
        };

        let record2 = SpcRecord {
            created_at: Utc::now(),
            uid: create_uuid7(),
            feature: "test2".to_string(),
            value: 2.0,
            entity_id: 0,
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
            uid: create_uuid7(),
            feature: "test".to_string(),
            bin_id: 1,
            bin_count: 1,
            entity_id: ENTITY_ID,
        };

        let record2 = PsiRecord {
            created_at: Utc::now(),
            uid: create_uuid7(),
            feature: "test2".to_string(),
            bin_id: 2,
            bin_count: 2,
            entity_id: ENTITY_ID,
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
                    uid: create_uuid7(),
                    feature: format!("test{j}"),
                    value: j as f64,
                    entity_id: ENTITY_ID,
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
                uid: create_uuid7(),
                time_interval: TimeInterval::FifteenMinutes,
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
                        uid: create_uuid7(),
                        feature: format!("feature{feature}"),
                        bin_id: bin,
                        bin_count: rand::rng().random_range(0..10) as i32,
                        entity_id: ENTITY_ID,
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
                uid: create_uuid7(),
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
                    entity_id: ENTITY_ID,
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
            entity_id: ENTITY_ID,
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
    async fn test_postgres_agent_eval_record_reschedule() {
        let pool = db_pool().await;

        let (uid, entity_id) = PostgresClient::create_entity(
            &pool,
            SPACE,
            NAME,
            VERSION,
            DriftType::Agent.to_string(),
        )
        .await
        .unwrap();

        let input = "This is a test input";
        let output = "This is a test response";

        for j in 0..10 {
            let context = serde_json::json!({
                "input": input,
                "response": output,
            });
            let record = EvalRecord {
                created_at: Utc::now() + chrono::Duration::microseconds(j as i64),
                context,
                status: Status::Pending,
                id: 0, // This will be set by the database
                uid: format!("test_{}", j),
                entity_uid: uid.clone(),
                entity_id,
                tags: vec!["key=value".to_string()],
                ..Default::default()
            };

            let boxed = BoxedEvalRecord::new(record);

            let result = PostgresClient::insert_agent_eval_record(&pool, boxed, &entity_id)
                .await
                .unwrap();

            assert_eq!(result.rows_affected(), 1);
        }

        let features = PostgresClient::get_agent_eval_records(&pool, None, None, &entity_id)
            .await
            .unwrap();
        assert_eq!(features.len(), 10);

        // get pending task
        let pending_tasks = PostgresClient::get_pending_agent_eval_record(&pool)
            .await
            .unwrap();

        // assert not empty
        assert!(pending_tasks.is_some());

        // get pending task with space, name, version
        let task_input = &pending_tasks.as_ref().unwrap().context["input"];
        assert_eq!(*task_input, "This is a test input".to_string());

        // reschedule task
        PostgresClient::reschedule_agent_eval_record(
            &pool,
            &pending_tasks.as_ref().unwrap().uid,
            Duration::seconds(30),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_postgres_agent_eval_record_insert_get() {
        let pool = db_pool().await;

        let (uid, entity_id) = PostgresClient::create_entity(
            &pool,
            SPACE,
            NAME,
            VERSION,
            DriftType::Agent.to_string(),
        )
        .await
        .unwrap();

        let input = "This is a test input";
        let output = "This is a test response";

        for j in 0..10 {
            let context = serde_json::json!({
                "input": input,
                "response": output,
            });
            let record = EvalRecord {
                created_at: Utc::now() + chrono::Duration::microseconds(j as i64),
                context,
                status: Status::Pending,
                id: 0, // This will be set by the database
                uid: format!("test_{}", j),
                entity_uid: uid.clone(),
                entity_id,
                ..Default::default()
            };

            let boxed = BoxedEvalRecord::new(record);

            let result = PostgresClient::insert_agent_eval_record(&pool, boxed, &entity_id)
                .await
                .unwrap();

            assert_eq!(result.rows_affected(), 1);
        }

        let features = PostgresClient::get_agent_eval_records(&pool, None, None, &entity_id)
            .await
            .unwrap();
        assert_eq!(features.len(), 10);

        // get pending task
        let pending_tasks = PostgresClient::get_pending_agent_eval_record(&pool)
            .await
            .unwrap();

        // assert not empty
        assert!(pending_tasks.is_some());

        // get pending task with space, name, version
        let task_input = &pending_tasks.as_ref().unwrap().context["input"];
        assert_eq!(*task_input, "This is a test input".to_string());

        // update pending task
        PostgresClient::update_agent_eval_record_status(
            &pool,
            &pending_tasks.unwrap(),
            Status::Processed,
            &(1_i64),
        )
        .await
        .unwrap();

        // query processed tasks
        let processed_tasks = PostgresClient::get_agent_eval_records(
            &pool,
            None,
            Some(Status::Processed),
            &entity_id,
        )
        .await
        .unwrap();

        // assert not empty
        assert_eq!(processed_tasks.len(), 1);
    }

    #[tokio::test]
    async fn test_postgres_agent_eval_record_pagination() {
        let pool = db_pool().await;

        let (uid, entity_id) = PostgresClient::create_entity(
            &pool,
            SPACE,
            NAME,
            VERSION,
            DriftType::Agent.to_string(),
        )
        .await
        .unwrap();

        let input = "This is a test input";
        let output = "This is a test response";

        // Insert 10 records with increasing timestamps
        for j in 0..10 {
            let context = serde_json::json!({
                "input": input,
                "response": output,
            });
            let record = EvalRecord {
                created_at: Utc::now() + chrono::Duration::microseconds(j as i64),
                context,
                status: Status::Pending,
                id: 0, // This will be set by the database
                uid: format!("test_{}", j),
                entity_uid: uid.clone(),
                entity_id: ENTITY_ID,
                ..Default::default()
            };

            let boxed = BoxedEvalRecord::new(record);

            let result = PostgresClient::insert_agent_eval_record(&pool, boxed, &entity_id)
                .await
                .unwrap();

            assert_eq!(result.rows_affected(), 1);
        }

        // ===== PAGE 1: Get first 5 records (newest) =====
        let params = EvalRecordPaginationRequest {
            status: None,
            limit: Some(5),
            cursor_created_at: None,
            cursor_id: None,
            direction: None,
            ..Default::default()
        };

        let page1 = PostgresClient::get_paginated_agent_eval_records(&pool, &params, &entity_id)
            .await
            .unwrap();

        assert_eq!(page1.items.len(), 5, "Page 1 should have 5 records");
        assert!(page1.has_next, "Should have next page");
        assert!(
            !page1.has_previous,
            "Should not have previous page (first page)"
        );
        assert!(page1.next_cursor.is_some(), "Should have next cursor");

        // First item should be the NEWEST record (highest ID)
        let page1_first = page1.items.first().unwrap();
        let page1_last = page1.items.last().unwrap();

        assert!(
            page1_first.created_at >= page1_last.created_at,
            "Page 1 should be sorted newest first (DESC)"
        );

        // ===== PAGE 2: Get next 5 records (older) =====
        let next_cursor = page1.next_cursor.unwrap();

        let params = EvalRecordPaginationRequest {
            status: None,
            limit: Some(5),
            cursor_created_at: Some(next_cursor.created_at),
            cursor_id: Some(next_cursor.id),
            direction: None,
            ..Default::default()
        };

        let page2 = PostgresClient::get_paginated_agent_eval_records(&pool, &params, &entity_id)
            .await
            .unwrap();

        assert_eq!(page2.items.len(), 5, "Page 2 should have 5 records");
        assert!(!page2.has_next, "Should not have next page (last page)");
        assert!(page2.has_previous, "Should have previous page");
        assert!(
            page2.previous_cursor.is_some(),
            "Should have previous cursor"
        );

        let page2_first = page2.items.first().unwrap();

        // Page 2 first item should be OLDER than Page 1 last item
        assert!(
            page2_first.created_at < page1_last.created_at
                || (page2_first.created_at == page1_last.created_at
                    && page2_first.id < page1_last.id),
            "Page 2 should start with records older than Page 1 last item"
        );

        // Verify we got all 10 records across both pages
        let all_ids: Vec<i64> = page1
            .items
            .iter()
            .chain(page2.items.iter())
            .map(|r| r.id)
            .collect();

        assert_eq!(all_ids.len(), 10, "Should have 10 unique records total");

        // All IDs should be unique
        let unique_ids: std::collections::HashSet<_> = all_ids.iter().collect();
        assert_eq!(unique_ids.len(), 10, "All IDs should be unique");

        // ===== BACKWARD PAGINATION TEST =====
        // Go back from page 2 to page 1
        let previous_cursor = page2.previous_cursor.unwrap();

        let params = EvalRecordPaginationRequest {
            status: None,
            limit: Some(5),
            cursor_created_at: Some(previous_cursor.created_at),
            cursor_id: Some(previous_cursor.id),
            direction: Some("previous".to_string()),
            ..Default::default()
        };

        let page1_again =
            PostgresClient::get_paginated_agent_eval_records(&pool, &params, &entity_id)
                .await
                .unwrap();

        assert_eq!(
            page1_again.items.len(),
            5,
            "Going back should return 5 records"
        );

        // Should get the same records as page 1
        assert_eq!(
            page1_again.items.first().unwrap().id,
            page1_first.id,
            "Should return to the same first record"
        );
    }

    #[tokio::test]
    async fn test_postgres_agent_eval_workflow_pagination() {
        let pool = db_pool().await;

        let (_uid, entity_id) = PostgresClient::create_entity(
            &pool,
            SPACE,
            NAME,
            VERSION,
            DriftType::Agent.to_string(),
        )
        .await
        .unwrap();

        // Insert 10 records with increasing timestamps
        for j in 0..10 {
            let record = AgentEvalWorkflowResult {
                created_at: Utc::now() + chrono::Duration::microseconds(j as i64),
                record_uid: format!("test_{}", j),
                entity_id,
                ..Default::default()
            };

            let result =
                PostgresClient::insert_agent_eval_workflow_record(&pool, &record, &entity_id)
                    .await
                    .unwrap();

            assert_eq!(result.rows_affected(), 1);
        }

        // ===== PAGE 1: Get first 5 records (newest) =====
        let params = EvalRecordPaginationRequest {
            status: None,
            limit: Some(5),
            cursor_created_at: None,
            cursor_id: None,
            direction: None,
            ..Default::default()
        };

        let page1 =
            PostgresClient::get_paginated_agent_eval_workflow_records(&pool, &params, &entity_id)
                .await
                .unwrap();

        assert_eq!(page1.items.len(), 5, "Page 1 should have 5 records");
        assert!(page1.has_next, "Should have next page");
        assert!(
            !page1.has_previous,
            "Should not have previous page (first page)"
        );
        assert!(page1.next_cursor.is_some(), "Should have next cursor");

        // First item should be the NEWEST record (highest ID)
        let page1_first = page1.items.first().unwrap();
        let page1_last = page1.items.last().unwrap();

        assert!(
            page1_first.created_at >= page1_last.created_at,
            "Page 1 should be sorted newest first (DESC)"
        );

        // ===== PAGE 2: Get next 5 records (older) =====
        let next_cursor = page1.next_cursor.unwrap();

        let params = EvalRecordPaginationRequest {
            status: None,
            limit: Some(5),
            cursor_created_at: Some(next_cursor.created_at),
            cursor_id: Some(next_cursor.id),
            direction: None,
            ..Default::default()
        };

        let page2 =
            PostgresClient::get_paginated_agent_eval_workflow_records(&pool, &params, &entity_id)
                .await
                .unwrap();

        assert_eq!(page2.items.len(), 5, "Page 2 should have 5 records");
        assert!(!page2.has_next, "Should not have next page (last page)");
        assert!(page2.has_previous, "Should have previous page");
        assert!(
            page2.previous_cursor.is_some(),
            "Should have previous cursor"
        );

        let page2_first = page2.items.first().unwrap();

        // Page 2 first item should be OLDER than Page 1 last item
        assert!(
            page2_first.created_at < page1_last.created_at
                || (page2_first.created_at == page1_last.created_at
                    && page2_first.id < page1_last.id),
            "Page 2 should start with records older than Page 1 last item"
        );

        // Verify we got all 10 records across both pages
        let all_ids: Vec<i64> = page1
            .items
            .iter()
            .chain(page2.items.iter())
            .map(|r| r.id)
            .collect();

        assert_eq!(all_ids.len(), 10, "Should have 10 unique records total");

        // All IDs should be unique
        let unique_ids: std::collections::HashSet<_> = all_ids.iter().collect();
        assert_eq!(unique_ids.len(), 10, "All IDs should be unique");

        // ===== BACKWARD PAGINATION TEST =====
        // Go back from page 2 to page 1
        let previous_cursor = page2.previous_cursor.unwrap();

        let params = EvalRecordPaginationRequest {
            status: None,
            limit: Some(5),
            cursor_created_at: Some(previous_cursor.created_at),
            cursor_id: Some(previous_cursor.id),
            direction: Some("previous".to_string()),
            ..Default::default()
        };

        let page1_again =
            PostgresClient::get_paginated_agent_eval_workflow_records(&pool, &params, &entity_id)
                .await
                .unwrap();

        assert_eq!(
            page1_again.items.len(),
            5,
            "Going back should return 5 records"
        );

        // Should get the same records as page 1
        assert_eq!(
            page1_again.items.first().unwrap().id,
            page1_first.id,
            "Should return to the same first record"
        );
    }

    #[tokio::test]
    async fn test_postgres_agent_task_result_insert_get() {
        let pool = db_pool().await;

        let timestamp = Utc::now();

        let (uid, entity_id) = PostgresClient::create_entity(
            &pool,
            SPACE,
            NAME,
            VERSION,
            DriftType::Agent.to_string(),
        )
        .await
        .unwrap();

        let mut records = Vec::new();
        for i in 0..2 {
            for j in 0..25 {
                let record = EvalTaskResult {
                    record_uid: format!("record_uid_{i}_{j}"),
                    created_at: Utc::now() + chrono::Duration::microseconds(j as i64),
                    start_time: Utc::now(),
                    end_time: Utc::now() + chrono::Duration::seconds(1),
                    entity_id,
                    task_id: format!("task{i}"),
                    task_type: scouter_types::agent::EvaluationTaskType::Assertion,
                    passed: true,
                    value: j as f64,
                    assertion: Assertion::FieldPath(Some(format!("field.path.{i}"))),
                    operator: scouter_types::agent::ComparisonOperator::Contains,
                    expected: Value::Null,
                    actual: Value::Null,
                    message: "All good".to_string(),
                    entity_uid: uid.clone(),
                    condition: false,
                    stage: 0_i32,
                };
                records.push(record);
            }
            let result =
                PostgresClient::insert_eval_task_results_batch(&pool, &records, &entity_id)
                    .await
                    .unwrap();
            assert_eq!(result.rows_affected(), 25);
        }

        let metrics = PostgresClient::get_agent_task_values(
            &pool,
            &timestamp,
            &["task1".to_string()],
            &entity_id,
        )
        .await
        .unwrap();

        assert_eq!(metrics.len(), 1);
        let binned_records = PostgresClient::get_binned_agent_task_values(
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

        let eval_task = PostgresClient::get_agent_eval_task(&pool, &records[0].record_uid)
            .await
            .unwrap();

        assert_eq!(eval_task[0].record_uid, records[0].record_uid);
    }

    #[tokio::test]
    async fn test_postgres_agent_workflow_result_insert_get() {
        let pool = db_pool().await;

        let timestamp = Utc::now();

        let (uid, entity_id) = PostgresClient::create_entity(
            &pool,
            SPACE,
            NAME,
            VERSION,
            DriftType::Agent.to_string(),
        )
        .await
        .unwrap();

        for i in 0..2 {
            for j in 0..25 {
                let record = AgentEvalWorkflowResult {
                    record_uid: format!("record_uid_{i}_{j}"),
                    created_at: Utc::now() + chrono::Duration::hours(i),
                    entity_id,
                    total_tasks: 10,
                    passed_tasks: 8,
                    failed_tasks: 2,
                    pass_rate: 0.8,
                    duration_ms: 1500,
                    entity_uid: uid.clone(),
                    execution_plan: ExecutionPlan::default(),
                    id: 0,
                };
                let result =
                    PostgresClient::insert_agent_eval_workflow_record(&pool, &record, &entity_id)
                        .await
                        .unwrap();
                assert_eq!(result.rows_affected(), 1);
            }
        }

        let metric = PostgresClient::get_agent_workflow_value(&pool, &timestamp, &entity_id)
            .await
            .unwrap();

        assert!(metric.is_some());

        let binned_records = PostgresClient::get_binned_agent_workflow_values(
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
        assert_eq!(binned_records.metrics.len(), 1);
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

        let tag_filter = vec![Tag {
            key: tags.first().unwrap().key.clone(),
            value: tags.first().unwrap().value.clone(),
        }];

        let entity_id = PostgresClient::get_entity_id_by_tags(&pool, "service", &tag_filter, false)
            .await
            .unwrap();

        assert_eq!(entity_id.first().unwrap(), &uid);
    }
}
