// Module for polling GenAI drift records that are "pending" and need to be processed
use crate::error::DriftError;
use chrono::Duration;
use scouter_evaluate::evaluate::GenAIEvaluator;
use scouter_sql::sql::traits::{GenAIDriftSqlLogic, ProfileSqlLogic, TraceSqlLogic};
use scouter_sql::PostgresClient;
use scouter_types::genai::{GenAIEvalProfile, GenAIEvalSet};
use scouter_types::sql::TraceSpan;
use scouter_types::{GenAIEvalRecord, Status, SCOUTER_QUEUE_RECORD};
use sqlx::{Pool, Postgres};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::sleep;
use tracing::{debug, error, instrument};

enum TraceSpanResult {
    Ready(Arc<Vec<TraceSpan>>),
    Reschedule,
    Failed,
}

#[instrument(skip_all)]
/// Helper function to wait for trace spans associated with a task UID
async fn wait_for_trace_spans(
    pool: &Pool<Postgres>,
    task_uid: &str,
    max_wait: Duration,
    initial_backoff: Duration,
) -> Result<Arc<Vec<TraceSpan>>, DriftError> {
    let start = chrono::Utc::now();
    let mut backoff = initial_backoff;

    let tags = vec![HashMap::from([
        ("key".to_string(), SCOUTER_QUEUE_RECORD.to_string()),
        ("value".to_string(), task_uid.to_string()),
    ])];

    loop {
        // todo: move this to a generic provider in case user wants to use their own trace storage
        match PostgresClient::get_spans_from_tags(pool, "trace", tags.clone(), false, None).await {
            Ok(spans) if !spans.is_empty() => {
                debug!("Found {} spans for task {}", spans.len(), task_uid);
                return Ok(Arc::new(spans));
            }
            Ok(_) => {
                if (chrono::Utc::now() - start) >= max_wait {
                    error!(
                        "Timeout waiting for trace spans after {:?} for task {}",
                        max_wait, task_uid
                    );
                    return Err(DriftError::TraceSpansNotAvailable(task_uid.to_string()));
                }

                debug!(
                    "No spans found yet for {}, waiting {:?} before retry",
                    task_uid, backoff
                );
                sleep(backoff.to_std().unwrap()).await;
                backoff = std::cmp::min(backoff * 2, Duration::seconds(5));
            }
            Err(e) => {
                error!("Error querying for trace spans: {:?}", e);
                if (chrono::Utc::now() - start) >= max_wait {
                    return Err(DriftError::SqlError(e));
                }
                sleep(backoff.to_std().unwrap()).await;
                backoff = std::cmp::min(backoff * 2, Duration::seconds(5));
            }
        }
    }
}

#[instrument(skip_all)]
async fn wait_for_trace_spans_with_reschedule(
    pool: &Pool<Postgres>,
    task: &GenAIEvalRecord,
    max_retries: &i32,
    trace_wait_timeout: Duration,
    trace_backoff: Duration,
    trace_reschedule_delay: Duration,
) -> Result<TraceSpanResult, DriftError> {
    let retry_count = task.retry_count;

    if retry_count >= *max_retries {
        return Ok(TraceSpanResult::Failed);
    }

    match wait_for_trace_spans(pool, &task.uid, trace_wait_timeout, trace_backoff).await {
        Ok(spans) => Ok(TraceSpanResult::Ready(spans)),
        Err(DriftError::TraceSpansNotAvailable(_)) => {
            PostgresClient::reschedule_genai_eval_record(pool, &task.uid, trace_reschedule_delay)
                .await?;
            Ok(TraceSpanResult::Reschedule)
        }
        Err(e) => Err(e),
    }
}

/// Poller struct for processing GenAI drift records
/// A few different things going on here:
/// 1. Poll the database for "pending" GenAI drift records
/// 2. For each record, check if trace spans are needed and available
/// 3. If spans are needed but not available, reschedule the record for later processing
/// 4. If spans are available or not needed, process the record using GenAIEvaluator
/// 5. Update the record status to "processed" or "failed" based on the outcome
pub struct GenAIPoller {
    db_pool: Pool<Postgres>,
    max_retries: i32,
    trace_wait_timeout: Duration,
    trace_backoff: Duration,
    trace_reschedule_delay: Duration,
}

impl GenAIPoller {
    pub fn new(
        db_pool: &Pool<Postgres>,
        max_retries: i32,
        trace_wait_timeout: Duration,
        trace_backoff: Duration,
        trace_reschedule_delay: Duration,
    ) -> Self {
        GenAIPoller {
            db_pool: db_pool.clone(),
            max_retries,
            trace_wait_timeout,
            trace_backoff,
            trace_reschedule_delay,
        }
    }

    #[instrument(skip_all)]
    pub async fn process_event_record(
        &mut self,
        record: &GenAIEvalRecord,
        profile: &GenAIEvalProfile,
        spans: Arc<Vec<TraceSpan>>,
    ) -> Result<GenAIEvalSet, DriftError> {
        debug!("Processing workflow");

        // create arc mutex for profile
        let profile = Arc::new(profile.clone());

        match GenAIEvaluator::process_event_record(record, profile, spans).await {
            Ok(result_set) => {
                // insert task results first
                PostgresClient::insert_eval_task_results_batch(
                    &self.db_pool,
                    &result_set.records,
                    &record.entity_id,
                )
                .await
                .inspect_err(|e| {
                    error!("Failed to insert LLM task results: {:?}", e);
                })?;

                // insert workflow record
                PostgresClient::insert_genai_eval_workflow_record(
                    &self.db_pool,
                    &result_set.inner,
                    &record.entity_id,
                )
                .await
                .inspect_err(|e| {
                    error!("Failed to insert GenAI workflow record: {:?}", e);
                })?;

                return Ok(result_set);
            }
            Err(e) => {
                error!("Failed to process drift record: {:?}", e);
                return Err(DriftError::GenAIEvaluatorError(e.to_string()));
            }
        };
    }

    #[instrument(skip_all)]
    pub async fn do_poll(&mut self) -> Result<bool, DriftError> {
        let task = PostgresClient::get_pending_genai_eval_record(&self.db_pool).await?;

        let Some(task) = task else {
            return Ok(false);
        };

        debug!("Processing genai drift record for profile: {}", task.uid);

        let mut genai_profile = if let Some(profile) =
            PostgresClient::get_drift_profile(&self.db_pool, &task.entity_id).await?
        {
            let genai_profile: GenAIEvalProfile =
                serde_json::from_value(profile).inspect_err(|e| {
                    error!("Failed to deserialize GenAI drift profile: {:?}", e);
                })?;
            genai_profile
        } else {
            error!("No GenAI drift profile found for {}", task.uid);
            return Ok(false);
        };

        let mut retry_count = 0;
        if let Some(workflow) = &mut genai_profile.workflow {
            workflow.reset_agents().await.inspect_err(|e| {
                error!("Failed to reset agents: {:?}", e);
            })?;
        }

        let spans = if genai_profile.has_trace_assertions() {
            match wait_for_trace_spans_with_reschedule(
                &self.db_pool,
                &task,
                &self.max_retries,
                self.trace_wait_timeout,
                self.trace_backoff,
                self.trace_reschedule_delay,
            )
            .await?
            {
                TraceSpanResult::Ready(spans) => spans,
                TraceSpanResult::Reschedule => {
                    debug!(
                        "Traces not yet available for task {}, rescheduled",
                        task.uid
                    );
                    return Ok(true);
                }
                TraceSpanResult::Failed => {
                    error!("Max retries exceeded for task {}", task.uid);
                    PostgresClient::update_genai_eval_record_status(
                        &self.db_pool,
                        &task,
                        Status::Failed,
                        &0,
                    )
                    .await?;
                    return Err(DriftError::TraceSpansNotAvailable(task.uid.clone()));
                }
            }
        } else {
            Arc::new(vec![])
        };

        loop {
            match self
                .process_event_record(&task, &genai_profile, spans.clone())
                .await
            {
                Ok(result_set) => {
                    PostgresClient::update_genai_eval_record_status(
                        &self.db_pool,
                        &task,
                        Status::Processed,
                        &result_set.inner.duration_ms,
                    )
                    .await?;
                    break;
                }
                Err(e) => {
                    error!(
                        "Failed to process drift record (attempt {}): {:?}",
                        retry_count + 1,
                        e
                    );

                    retry_count += 1;
                    if retry_count >= self.max_retries {
                        // Update the record status to error
                        PostgresClient::update_genai_eval_record_status(
                            &self.db_pool,
                            &task,
                            Status::Failed,
                            &0,
                        )
                        .await?;
                        return Err(DriftError::GenAIEvaluatorError(e.to_string()));
                    } else {
                        // Exponential backoff before retrying
                        let val = 100 * 2_i64.pow(retry_count as u32);
                        sleep(Duration::milliseconds(val).to_std()?).await;
                    }
                }
            }
        }

        Ok(true)
    }

    #[instrument(skip_all)]
    pub async fn poll_for_tasks(&mut self) -> Result<(), DriftError> {
        let result = self.do_poll().await;

        // silent error handling
        match result {
            Ok(true) => {
                debug!("Successfully processed drift record");
                Ok(())
            }
            Ok(false) => {
                sleep(Duration::seconds(1).to_std()?).await;
                Ok(())
            }
            Err(e) => {
                error!("Error processing drift record: {:?}", e);
                Ok(())
            }
        }
    }
}
