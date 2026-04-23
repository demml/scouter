use crate::error::DriftError;
use chrono::{DateTime, Duration, Utc};
use scouter_dataframe::parquet::tracing::dispatch::{
    DispatchCandidate, DispatchCursor, DispatchEventType, TraceDispatchRecord, TraceDispatchService,
};
use scouter_sql::sql::aggregator::get_trace_dispatch_service;
use scouter_sql::sql::traits::{AgentDriftSqlLogic, SyntheticInsertOutcome};
use scouter_sql::sql::utils::UuidBytea;
use scouter_sql::PostgresClient;
use sqlx::{Pool, Postgres};
use std::collections::{HashMap, HashSet};
use tokio::sync::RwLock;
use tracing::{debug, error, instrument, warn};

#[derive(Default)]
struct ProfileCache {
    by_entity_uid: HashMap<[u8; 16], i32>,
    missing_entity_uids: HashSet<[u8; 16]>,
    refreshed_at: Option<DateTime<Utc>>,
}

pub struct TraceEvalPoller {
    db_pool: Pool<Postgres>,
    lookback: Duration,
    dispatch_page_size: usize,
    profile_cache_ttl: Duration,
    max_pages_per_poll: usize,
    profile_cache: RwLock<ProfileCache>,
}

impl TraceEvalPoller {
    pub fn new(
        db_pool: &Pool<Postgres>,
        lookback: Duration,
        dispatch_page_size: usize,
        profile_cache_ttl: Duration,
    ) -> Self {
        TraceEvalPoller {
            db_pool: db_pool.clone(),
            lookback,
            dispatch_page_size: dispatch_page_size.max(1),
            profile_cache_ttl,
            max_pages_per_poll: 500,
            profile_cache: RwLock::new(ProfileCache::default()),
        }
    }

    fn should_refresh_profiles(
        now: DateTime<Utc>,
        last_refresh: Option<DateTime<Utc>>,
        profile_cache_ttl: Duration,
    ) -> bool {
        match last_refresh {
            None => true,
            Some(last) => now.signed_duration_since(last) >= profile_cache_ttl,
        }
    }

    #[instrument(skip_all)]
    async fn refresh_profiles_if_needed(&self, force: bool) -> Result<(), DriftError> {
        let now = Utc::now();

        if !force {
            let cache = self.profile_cache.read().await;
            if !Self::should_refresh_profiles(now, cache.refreshed_at, self.profile_cache_ttl) {
                return Ok(());
            }
        }

        let profiles = PostgresClient::get_active_agent_profiles(&self.db_pool).await?;
        let mut by_entity_uid = HashMap::new();

        for (entity_id, profile) in profiles {
            if !profile.has_trace_assertions() {
                continue;
            }
            match UuidBytea::from_uuid(&profile.config.uid) {
                Ok(uid) => {
                    by_entity_uid.insert(*uid.as_bytes(), entity_id);
                }
                Err(e) => {
                    warn!(
                        entity_id,
                        profile_uid = %profile.config.uid,
                        error = %e,
                        "Skipping trace-eval profile with invalid uid"
                    );
                }
            }
        }

        let mut cache = self.profile_cache.write().await;
        cache.by_entity_uid = by_entity_uid;
        cache.missing_entity_uids.clear();
        cache.refreshed_at = Some(now);
        Ok(())
    }

    #[instrument(skip_all)]
    async fn resolve_entity_id(&self, entity_uid: &[u8; 16]) -> Result<Option<i32>, DriftError> {
        self.refresh_profiles_if_needed(false).await?;
        let mut cache = self.profile_cache.write().await;
        if let Some(entity_id) = cache.by_entity_uid.get(entity_uid) {
            return Ok(Some(*entity_id));
        }
        if cache.missing_entity_uids.contains(entity_uid) {
            return Ok(None);
        }
        cache.missing_entity_uids.insert(*entity_uid);
        Ok(None)
    }

    #[instrument(skip_all)]
    async fn process_dispatch_candidates(
        &self,
        dispatch_service: &TraceDispatchService,
        items: &[DispatchCandidate],
    ) -> Result<usize, DriftError> {
        if items.is_empty() {
            return Ok(0);
        }

        let mut inserted = 0usize;
        let mut skipped = 0usize;
        let mut deferred = 0usize;
        let mut ack_records = Vec::new();
        let now = Utc::now();

        for item in items {
            let Some(entity_id) = self.resolve_entity_id(&item.entity_uid).await? else {
                skipped += 1;
                continue;
            };

            let outcome = PostgresClient::insert_synthetic_eval_record(
                &self.db_pool,
                entity_id,
                &item.trace_id,
            )
            .await?;

            match outcome {
                SyntheticInsertOutcome::Inserted => {
                    inserted += 1;
                    ack_records.push(TraceDispatchRecord {
                        trace_id: item.trace_id,
                        entity_uid: item.entity_uid,
                        start_time: item.start_time,
                        event_type: DispatchEventType::Ack,
                        created_at: now,
                    });
                }
                SyntheticInsertOutcome::AlreadyExists => {
                    ack_records.push(TraceDispatchRecord {
                        trace_id: item.trace_id,
                        entity_uid: item.entity_uid,
                        start_time: item.start_time,
                        event_type: DispatchEventType::Ack,
                        created_at: now,
                    });
                }
                SyntheticInsertOutcome::RetryLater => {
                    deferred += 1;
                }
            }
        }

        if !ack_records.is_empty() {
            dispatch_service
                .write_records(ack_records)
                .await
                .map_err(|e| {
                    DriftError::AgentEvaluatorError(format!(
                        "Failed to write dispatch acknowledgements: {}",
                        e
                    ))
                })?;
        }

        if skipped > 0 {
            debug!(
                skipped,
                "Skipped dispatch candidates with no active trace-assertion profile"
            );
        }
        if deferred > 0 {
            debug!(
                deferred,
                "Deferred dispatch candidates due to synthetic insert contention"
            );
        }
        Ok(inserted)
    }

    #[instrument(skip_all)]
    pub async fn do_poll(&self) -> Result<bool, DriftError> {
        let dispatch_service = get_trace_dispatch_service().ok_or_else(|| {
            DriftError::AgentEvaluatorError("TraceDispatchService not initialized".to_string())
        })?;
        let lookback_start = Utc::now() - self.lookback;

        let mut cursor: Option<DispatchCursor> = None;
        let mut total = 0usize;
        let mut pages = 0usize;

        loop {
            let page = dispatch_service
                .query_service
                .get_pending_dispatch(lookback_start, self.dispatch_page_size, cursor.as_ref())
                .await
                .map_err(|e| {
                    DriftError::AgentEvaluatorError(format!("Dispatch outbox query failed: {}", e))
                })?;

            if page.items.is_empty() {
                break;
            }

            total += self
                .process_dispatch_candidates(dispatch_service.as_ref(), &page.items)
                .await?;
            pages += 1;

            if !page.has_next {
                break;
            }

            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }

            if pages >= self.max_pages_per_poll {
                warn!(
                    pages,
                    max_pages_per_poll = self.max_pages_per_poll,
                    "TraceEvalPoller reached page limit for a single cycle; continuing next poll"
                );
                break;
            }
        }

        if total > 0 {
            debug!(
                inserted = total,
                pages, "Inserted synthetic eval records from trace dispatch outbox"
            );
        }

        Ok(total > 0)
    }

    #[instrument(skip_all)]
    pub async fn poll_for_tasks(&self) -> Result<(), DriftError> {
        if let Err(e) = self.do_poll().await {
            error!(error = %e, "TraceEvalPoller: cycle failed");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use potato_head::create_uuid7;
    use scouter_dataframe::parquet::tracing::dispatch::{
        DispatchEventType, TraceDispatchRecord, TraceDispatchService,
    };
    use scouter_dataframe::parquet::tracing::service::init_trace_span_service;
    use scouter_settings::{DatabaseSettings, ObjectStorageSettings};
    use scouter_sql::sql::aggregator::init_trace_dispatch_service;
    use scouter_sql::sql::traits::{EntitySqlLogic, ProfileSqlLogic};
    use scouter_types::agent::{
        AgentAlertConfig, AgentEvalConfig, AgentEvalProfile, ComparisonOperator,
        EvaluationTaskType, EvaluationTasks, TraceAssertion, TraceAssertionTask,
    };
    use scouter_types::{DriftProfile, DriftType, EvalRecordSource, ProfileArgs};
    use semver::Version;
    use serde_json::Value;
    use sqlx::{Pool, Postgres};
    use std::sync::Arc;
    use tokio::time::sleep;

    #[test]
    fn should_refresh_profiles_when_never_refreshed() {
        assert!(TraceEvalPoller::should_refresh_profiles(
            Utc::now(),
            None,
            Duration::seconds(60)
        ));
    }

    #[test]
    fn should_refresh_profiles_after_ttl() {
        let now = Utc::now();
        assert!(TraceEvalPoller::should_refresh_profiles(
            now,
            Some(now - Duration::seconds(61)),
            Duration::seconds(60)
        ));
        assert!(!TraceEvalPoller::should_refresh_profiles(
            now,
            Some(now - Duration::seconds(30)),
            Duration::seconds(60)
        ));
    }

    async fn cleanup(pool: &Pool<Postgres>) {
        sqlx::raw_sql(
            r#"
                DELETE FROM scouter.agent_eval_task;
                DELETE FROM scouter.agent_eval_workflow;
                DELETE FROM scouter.agent_eval_record;
                DELETE FROM scouter.drift_profile;
                DELETE FROM scouter.drift_entities;
            "#,
        )
        .execute(pool)
        .await
        .unwrap();
    }

    async fn db_pool() -> Pool<Postgres> {
        let pool = PostgresClient::create_db_pool(&DatabaseSettings::default())
            .await
            .unwrap();
        cleanup(&pool).await;
        pool
    }

    fn test_storage_settings() -> ObjectStorageSettings {
        let mut storage_path = std::env::temp_dir();
        storage_path.push(format!("scouter-trace-eval-dispatch-{}", create_uuid7()));
        std::fs::create_dir_all(&storage_path).unwrap();
        ObjectStorageSettings {
            storage_uri: storage_path.to_string_lossy().to_string(),
            ..ObjectStorageSettings::default()
        }
    }

    async fn init_dispatch_service() -> Arc<TraceDispatchService> {
        let storage_settings = test_storage_settings();
        let trace_service = init_trace_span_service(&storage_settings, 24, Some(1), Some(7), 1)
            .await
            .unwrap();
        let dispatch_service = Arc::new(
            TraceDispatchService::new(
                &trace_service.object_store,
                trace_service.ctx.clone(),
                trace_service.catalog.clone(),
            )
            .await
            .unwrap(),
        );
        init_trace_dispatch_service(dispatch_service.clone()).unwrap();
        dispatch_service
    }

    fn make_trace_task() -> TraceAssertionTask {
        TraceAssertionTask {
            id: "trace-span-count".to_string(),
            assertion: TraceAssertion::TraceSpanCount {},
            operator: ComparisonOperator::GreaterThanOrEqual,
            expected_value: Value::Number(0_i64.into()),
            description: Some("trace assertion required for dispatch path".to_string()),
            task_type: EvaluationTaskType::TraceAssertion,
            depends_on: vec![],
            condition: false,
            result: None,
        }
    }

    async fn insert_active_trace_profile(
        pool: &Pool<Postgres>,
        space: &str,
        name: &str,
    ) -> (String, i32) {
        let config =
            AgentEvalConfig::new(space, name, "0.1.0", 1.0, AgentAlertConfig::default(), None)
                .unwrap();

        let profile = AgentEvalProfile::new(
            config,
            EvaluationTasks::new().add_task(make_trace_task()).build(),
        )
        .await
        .unwrap();

        let drift_profile = DriftProfile::Agent(profile);
        let profile_args = ProfileArgs {
            space: space.to_string(),
            name: name.to_string(),
            version: Some("0.1.0".to_string()),
            schedule: "* * * * * *".to_string(),
            scouter_version: "0.1.0".to_string(),
            drift_type: DriftType::Agent,
        };

        let entity_uid = PostgresClient::insert_drift_profile(
            pool,
            &drift_profile,
            &profile_args,
            &Version::new(0, 1, 0),
            &true,
            &true,
        )
        .await
        .unwrap();
        let entity_id = PostgresClient::get_entity_id_from_uid(pool, &entity_uid)
            .await
            .unwrap();

        (entity_uid, entity_id)
    }

    fn uid_to_bytes(uid: &str) -> [u8; 16] {
        *UuidBytea::from_uuid(uid).unwrap().as_bytes()
    }

    #[tokio::test]
    async fn do_poll_inserts_single_record_for_duplicate_candidates_and_acks() {
        let pool = db_pool().await;
        let dispatch_service = init_dispatch_service().await;
        let (profile_uid, entity_id) =
            insert_active_trace_profile(&pool, "scouter", "trace-dup-ack").await;
        let active_profiles = PostgresClient::get_active_agent_profiles(&pool)
            .await
            .unwrap();
        assert!(
            active_profiles.iter().any(|(_, profile)| {
                profile.config.uid == profile_uid && profile.has_trace_assertions()
            }),
            "active profile mismatch: expected_uid={}, active={:?}",
            profile_uid,
            active_profiles
                .iter()
                .map(|(_, profile)| (profile.config.uid.clone(), profile.has_trace_assertions()))
                .collect::<Vec<_>>()
        );

        let entity_uid = uid_to_bytes(&profile_uid);
        let trace_id = [11_u8; 16];
        let now = Utc::now();

        dispatch_service
            .write_records(vec![
                TraceDispatchRecord {
                    trace_id,
                    entity_uid,
                    start_time: now,
                    event_type: DispatchEventType::Candidate,
                    created_at: now,
                },
                TraceDispatchRecord {
                    trace_id,
                    entity_uid,
                    start_time: now,
                    event_type: DispatchEventType::Candidate,
                    created_at: now,
                },
            ])
            .await
            .unwrap();

        let pending_before = dispatch_service
            .query_service
            .get_pending_dispatch(now - Duration::minutes(10), 50, None)
            .await
            .unwrap();
        assert_eq!(pending_before.items.len(), 1);

        let poller = TraceEvalPoller::new(&pool, Duration::hours(2), 100, Duration::seconds(60));
        poller.do_poll().await.unwrap();
        poller.do_poll().await.unwrap();

        let records = PostgresClient::get_agent_eval_records(&pool, None, None, &entity_id)
            .await
            .unwrap();
        assert_eq!(records.len(), 1);
        assert!(matches!(
            records[0].record_source,
            EvalRecordSource::TraceDispatch
        ));
        let stored_trace = records[0].trace_id.as_ref().map(|id| *id.as_bytes());
        assert_eq!(stored_trace, Some(trace_id));

        let pending = dispatch_service
            .query_service
            .get_pending_dispatch(now - Duration::minutes(10), 50, None)
            .await
            .unwrap();
        assert!(pending.items.is_empty());
        dispatch_service.signal_shutdown().await;
    }

    #[tokio::test]
    async fn do_poll_acks_existing_synthetic_record_without_duplicate_insert() {
        let pool = db_pool().await;
        let dispatch_service = init_dispatch_service().await;
        let (profile_uid, entity_id) =
            insert_active_trace_profile(&pool, "scouter", "trace-existing").await;
        let active_profiles = PostgresClient::get_active_agent_profiles(&pool)
            .await
            .unwrap();
        assert!(
            active_profiles.iter().any(|(_, profile)| {
                profile.config.uid == profile_uid && profile.has_trace_assertions()
            }),
            "active profile mismatch: expected_uid={}, active={:?}",
            profile_uid,
            active_profiles
                .iter()
                .map(|(_, profile)| (profile.config.uid.clone(), profile.has_trace_assertions()))
                .collect::<Vec<_>>()
        );

        let entity_uid = uid_to_bytes(&profile_uid);
        let trace_id = [22_u8; 16];
        let now = Utc::now();

        let initial = PostgresClient::insert_synthetic_eval_record(&pool, entity_id, &trace_id)
            .await
            .unwrap();
        assert_eq!(initial, SyntheticInsertOutcome::Inserted);

        dispatch_service
            .write_records(vec![TraceDispatchRecord {
                trace_id,
                entity_uid,
                start_time: now,
                event_type: DispatchEventType::Candidate,
                created_at: now,
            }])
            .await
            .unwrap();

        let poller = TraceEvalPoller::new(&pool, Duration::hours(2), 100, Duration::seconds(60));
        poller.do_poll().await.unwrap();

        let records = PostgresClient::get_agent_eval_records(&pool, None, None, &entity_id)
            .await
            .unwrap();
        assert_eq!(records.len(), 1);
        assert!(matches!(
            records[0].record_source,
            EvalRecordSource::TraceDispatch
        ));

        let pending = dispatch_service
            .query_service
            .get_pending_dispatch(now - Duration::minutes(10), 50, None)
            .await
            .unwrap();
        assert!(pending.items.is_empty());
        dispatch_service.signal_shutdown().await;
    }

    #[tokio::test]
    async fn do_poll_retries_unknown_entity_after_profile_cache_ttl() {
        let pool = db_pool().await;
        let dispatch_service = init_dispatch_service().await;
        let unknown_profile_uid = create_uuid7();
        let unknown_entity_uid = uid_to_bytes(&unknown_profile_uid);
        let unknown_trace_id = [33_u8; 16];
        let now = Utc::now();

        dispatch_service
            .write_records(vec![TraceDispatchRecord {
                trace_id: unknown_trace_id,
                entity_uid: unknown_entity_uid,
                start_time: now,
                event_type: DispatchEventType::Candidate,
                created_at: now,
            }])
            .await
            .unwrap();

        let poller =
            TraceEvalPoller::new(&pool, Duration::hours(2), 100, Duration::milliseconds(100));
        assert!(!poller.do_poll().await.unwrap());

        let first_pending = dispatch_service
            .query_service
            .get_pending_dispatch(now - Duration::minutes(10), 50, None)
            .await
            .unwrap();
        assert_eq!(first_pending.items.len(), 1);

        let (new_profile_uid, entity_id) =
            insert_active_trace_profile(&pool, "scouter", "trace-cache-refresh").await;
        let new_entity_uid = uid_to_bytes(&new_profile_uid);
        let new_trace_id = [34_u8; 16];

        dispatch_service
            .write_records(vec![TraceDispatchRecord {
                trace_id: new_trace_id,
                entity_uid: new_entity_uid,
                start_time: now,
                event_type: DispatchEventType::Candidate,
                created_at: now,
            }])
            .await
            .unwrap();

        // Negative cache should block immediate re-resolution until TTL expires.
        poller.do_poll().await.unwrap();

        sleep(std::time::Duration::from_millis(120)).await;
        poller.do_poll().await.unwrap();

        let records = PostgresClient::get_agent_eval_records(&pool, None, None, &entity_id)
            .await
            .unwrap();
        assert_eq!(records.len(), 1);
        assert!(matches!(
            records[0].record_source,
            EvalRecordSource::TraceDispatch
        ));

        let pending = dispatch_service
            .query_service
            .get_pending_dispatch(now - Duration::minutes(10), 50, None)
            .await
            .unwrap();
        assert_eq!(pending.items.len(), 1);
        assert_eq!(pending.items[0].trace_id, unknown_trace_id);
        dispatch_service.signal_shutdown().await;
    }
}
