// Module for polling GenAI drift records that are "pending" and need to be processed
use crate::error::DriftError;
use scouter_evaluate::evaluate::GenAIEvaluator;
use scouter_sql::sql::traits::{GenAIDriftSqlLogic, ProfileSqlLogic, TraceSqlLogic};
use scouter_sql::PostgresClient;
use scouter_types::genai::{GenAIEvalProfile, GenAIEvalSet};
use scouter_types::sql::TraceSpan;
use scouter_types::{GenAIEvalRecord, Status, SCOUTER_QUEUE_RECORD};
use sqlx::{Pool, Postgres};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, instrument};
pub struct GenAIPoller {
    db_pool: Pool<Postgres>,
    max_retries: usize,
}

impl GenAIPoller {
    pub fn new(db_pool: &Pool<Postgres>, max_retries: usize) -> Self {
        GenAIPoller {
            db_pool: db_pool.clone(),
            max_retries,
        }
    }

    #[instrument(skip_all)]
    pub async fn process_event_record(
        &mut self,
        record: &GenAIEvalRecord,
        profile: &GenAIEvalProfile,
        spans: Vec<TraceSpan>,
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
        // Get task from the database (query uses skip lock to pull task and update to processing)
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

        // if genai_profile has trace_tasks, query for trace
        // todo - cleanup later
        let spans = if genai_profile.has_trace_assertions() {
            let tags = vec![HashMap::from([
                ("key".to_string(), SCOUTER_QUEUE_RECORD.to_string()),
                ("value".to_string(), task.uid.clone()),
            ])];

            match PostgresClient::get_spans_from_tags(&self.db_pool, "trace", tags, false, None)
                .await
                .inspect_err(|e| {
                    error!("Failed to get spans for trace tasks: {:?}", e);
                }) {
                Ok(spans) => spans,
                Err(_) => {
                    error!("No spans found for trace tasks for {}", task.uid);
                    vec![]
                }
            }
        } else {
            vec![]
        };

        loop {
            match self
                .process_event_record(&task, &genai_profile, spans)
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
                        sleep(Duration::from_millis(100 * 2_u64.pow(retry_count as u32))).await;
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
                sleep(Duration::from_secs(1)).await;
                Ok(())
            }
            Err(e) => {
                error!("Error processing drift record: {:?}", e);
                Ok(())
            }
        }
    }
}
