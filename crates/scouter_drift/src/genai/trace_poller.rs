use crate::error::DriftError;
use chrono::Duration;
use scouter_sql::sql::aggregator::get_trace_summary_service;
use scouter_sql::sql::traits::AgentDriftSqlLogic;
use scouter_sql::PostgresClient;
use scouter_types::agent::AgentEvalProfile;
use scouter_types::sql::TraceFilters;
use scouter_types::TraceId;
use sqlx::{Pool, Postgres};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, instrument};

pub struct TraceEvalPoller {
    db_pool: Pool<Postgres>,
    lookback: Duration,
    poll_interval: std::time::Duration,
}

impl TraceEvalPoller {
    pub fn new(
        db_pool: &Pool<Postgres>,
        lookback: Duration,
        poll_interval: std::time::Duration,
    ) -> Self {
        TraceEvalPoller {
            db_pool: db_pool.clone(),
            lookback,
            poll_interval,
        }
    }

    pub fn with_defaults(db_pool: &Pool<Postgres>) -> Self {
        Self::new(
            db_pool,
            Duration::hours(2),
            std::time::Duration::from_secs(30),
        )
    }

    #[instrument(skip_all)]
    async fn process_profile(
        &self,
        entity_id: i32,
        profile: &AgentEvalProfile,
    ) -> Result<usize, DriftError> {
        let entity_uid = &profile.config.uid;

        let summary_service = get_trace_summary_service().ok_or_else(|| {
            DriftError::AgentEvaluatorError("TraceSummaryService not initialized".to_string())
        })?;

        let lookback_start = chrono::Utc::now() - self.lookback;

        let filters = TraceFilters {
            entity_uid: Some(entity_uid.clone()),
            start_time: Some(lookback_start),
            ..Default::default()
        };

        let response = summary_service
            .query_service
            .get_paginated_traces(&filters)
            .await
            .map_err(|e| DriftError::AgentEvaluatorError(format!("Delta Lake query failed: {}", e)))?;

        if response.items.is_empty() {
            debug!(entity_id, "No trace summaries found in lookback window");
            return Ok(0);
        }

        let known_ids =
            PostgresClient::get_known_trace_ids_for_entity(&self.db_pool, entity_id, lookback_start)
                .await?;

        let mut inserted = 0usize;

        for item in &response.items {
            if !item.queue_ids.is_empty() {
                debug!(
                    trace_id = %item.trace_id,
                    "Skipping trace with queue_ids — queue path handles it"
                );
                continue;
            }

            if known_ids.contains(&item.trace_id) {
                debug!(trace_id = %item.trace_id, "Trace already known, skipping");
                continue;
            }

            let trace_id_bytes = TraceId::hex_to_bytes(&item.trace_id).map_err(|e| {
                DriftError::AgentEvaluatorError(format!(
                    "Invalid trace_id hex '{}': {}",
                    item.trace_id, e
                ))
            })?;

            PostgresClient::insert_synthetic_eval_record(&self.db_pool, entity_id, &trace_id_bytes)
                .await?;

            inserted += 1;
        }

        if inserted > 0 {
            debug!(entity_id, inserted, "Inserted synthetic eval records");
        }

        Ok(inserted)
    }

    #[instrument(skip_all)]
    pub async fn do_poll(&self) -> Result<bool, DriftError> {
        let profiles = PostgresClient::get_active_agent_profiles(&self.db_pool).await?;

        let mut total = 0usize;

        for (entity_id, profile) in profiles
            .into_iter()
            .filter(|(_, p)| p.has_trace_assertions())
        {
            match self.process_profile(entity_id, &profile).await {
                Ok(n) => total += n,
                Err(e) => {
                    error!(entity_id, error = %e, "TraceEvalPoller: profile failed, skipping");
                }
            }
        }

        Ok(total > 0)
    }

    #[instrument(skip_all)]
    pub async fn poll_for_tasks(&self, shutdown: CancellationToken) {
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    debug!("TraceEvalPoller shutting down");
                    break;
                }
                _ = tokio::time::sleep(self.poll_interval) => {
                    if let Err(e) = self.do_poll().await {
                        error!(error = %e, "TraceEvalPoller: cycle failed");
                    }
                }
            }
        }
    }
}
