use crate::error::DriftError;
use chrono::{DateTime, Duration, Utc};
use scouter_dataframe::parquet::tracing::dispatch::{
    DispatchCandidate, DispatchCursor, DispatchEventType, TraceDispatchRecord, TraceDispatchService,
};
use scouter_sql::sql::aggregator::get_trace_dispatch_service;
use scouter_sql::sql::traits::AgentDriftSqlLogic;
use scouter_sql::sql::utils::UuidBytea;
use scouter_sql::PostgresClient;
use sqlx::{Pool, Postgres};
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{debug, error, instrument, warn};

#[derive(Default)]
struct ProfileCache {
    by_entity_uid: HashMap<[u8; 16], i32>,
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
        cache.refreshed_at = Some(now);
        Ok(())
    }

    #[instrument(skip_all)]
    async fn resolve_entity_id(&self, entity_uid: &[u8; 16]) -> Result<Option<i32>, DriftError> {
        self.refresh_profiles_if_needed(false).await?;
        {
            let cache = self.profile_cache.read().await;
            if let Some(entity_id) = cache.by_entity_uid.get(entity_uid) {
                return Ok(Some(*entity_id));
            }
        }

        self.refresh_profiles_if_needed(true).await?;
        let cache = self.profile_cache.read().await;
        Ok(cache.by_entity_uid.get(entity_uid).copied())
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
        let mut ack_records = Vec::new();
        let now = Utc::now();

        for item in items {
            let Some(entity_id) = self.resolve_entity_id(&item.entity_uid).await? else {
                skipped += 1;
                continue;
            };

            let was_inserted = PostgresClient::insert_synthetic_eval_record(
                &self.db_pool,
                entity_id,
                &item.trace_id,
            )
            .await?;

            // Mark as dispatched by appending an ack event for the same key.
            ack_records.push(TraceDispatchRecord {
                trace_id: item.trace_id,
                entity_uid: item.entity_uid,
                start_time: item.start_time,
                event_type: DispatchEventType::Ack,
                created_at: now,
            });

            if was_inserted {
                inserted += 1;
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
}
