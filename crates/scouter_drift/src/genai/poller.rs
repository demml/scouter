// Module for polling GenAI drift records that are "pending" and need to be processed
use crate::error::DriftError;
use crate::genai::evaluator::GenAIEvaluator;
use potato_head::prompt_types::Score;
use scouter_sql::sql::traits::{GenAIDriftSqlLogic, ProfileSqlLogic};
use scouter_sql::PostgresClient;
use scouter_types::genai::GenAIDriftProfile;
use scouter_types::{GenAITaskRecord, Status};
use sqlx::{Pool, Postgres};
use std::collections::HashMap;
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
    pub async fn process_drift_record(
        &mut self,
        record: &GenAITaskRecord,
        profile: &GenAIDriftProfile,
    ) -> Result<(HashMap<String, Score>, Option<i32>), DriftError> {
        debug!("Processing workflow");

        match GenAIEvaluator::process_drift_record(record, profile).await {
            Ok((metrics, score_map, workflow_duration)) => {
                PostgresClient::insert_genai_metric_values_batch(
                    &self.db_pool,
                    &metrics,
                    &record.entity_id,
                )
                .await
                .inspect_err(|e| {
                    error!("Failed to insert LLM metric values: {:?}", e);
                })?;

                return Ok((score_map, workflow_duration));
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
        let task = PostgresClient::get_pending_genai_event_record(&self.db_pool).await?;

        let Some(mut task) = task else {
            return Ok(false);
        };

        debug!("Processing genai drift record for profile: {}", task.uid);

        let mut genai_profile = if let Some(profile) =
            PostgresClient::get_drift_profile(&self.db_pool, &task.entity_id).await?
        {
            let genai_profile: GenAIDriftProfile =
                serde_json::from_value(profile).inspect_err(|e| {
                    error!("Failed to deserialize GenAI drift profile: {:?}", e);
                })?;
            genai_profile
        } else {
            error!("No GenAI drift profile found for {}", task.uid);
            return Ok(false);
        };
        let mut retry_count = 0;

        genai_profile
            .workflow
            .reset_agents()
            .await
            .inspect_err(|e| {
                error!("Failed to reset agents: {:?}", e);
            })?;

        loop {
            match self.process_drift_record(&task, &genai_profile).await {
                Ok((result, workflow_duration)) => {
                    task.score = serde_json::to_value(result).inspect_err(|e| {
                        error!("Failed to serialize score map: {:?}", e);
                    })?;

                    PostgresClient::update_genai_event_record_status(
                        &self.db_pool,
                        &task,
                        Status::Processed,
                        workflow_duration,
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
                        PostgresClient::update_genai_event_record_status(
                            &self.db_pool,
                            &task,
                            Status::Failed,
                            None,
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
