// Module for polling LLM drift records that are "pending" and need to be processed
use crate::error::DriftError;
use crate::llm::evaluator::LLMEvaluator;
use potato_head::Score;
use scouter_sql::sql::traits::{LLMDriftSqlLogic, ProfileSqlLogic};
use scouter_sql::PostgresClient;
use scouter_types::llm::LLMDriftProfile;
use scouter_types::{DriftType, GetProfileRequest, LLMRecord, Status};
use sqlx::{Pool, Postgres};
use std::collections::HashMap;
use tracing::{debug, error, info, instrument};

pub struct LLMPoller {
    db_pool: Pool<Postgres>,
}

impl LLMPoller {
    pub fn new(db_pool: &Pool<Postgres>) -> Self {
        LLMPoller {
            db_pool: db_pool.clone(),
        }
    }

    #[instrument(skip_all)]
    pub async fn process_drift_record(
        &mut self,
        record: &LLMRecord,
        profile: &LLMDriftProfile,
    ) -> Result<HashMap<String, Score>, DriftError> {
        debug!("Processing workflow");

        match LLMEvaluator::process_drift_record(record, profile).await {
            Ok((metrics, score_map)) => {
                PostgresClient::insert_llm_metric_values_batch(&self.db_pool, &metrics)
                    .await
                    .inspect_err(|e| {
                        error!("Failed to insert LLM metric values: {:?}", e);
                    })?;

                return Ok(score_map);
            }
            Err(e) => {
                error!("Failed to process drift record: {:?}", e);
                return Err(DriftError::LLMEvaluatorError(e.to_string()));
            }
        };
    }

    #[instrument(skip_all)]
    pub async fn do_poll(&mut self) -> Result<bool, DriftError> {
        // Get task from the database (query uses skip lock to pull task and update to processing)
        let task = PostgresClient::get_pending_llm_drift_record(&self.db_pool).await?;

        let Some(mut task) = task else {
            return Ok(false);
        };

        info!(
            "Processing llm drift record for profile: {}/{}/{}",
            task.space, task.name, task.version
        );

        // get profile
        let request = GetProfileRequest {
            space: task.space.clone(),
            name: task.name.clone(),
            version: task.version.clone(),
            drift_type: DriftType::LLM,
        };
        let llm_profile = if let Some(profile) =
            PostgresClient::get_drift_profile(&self.db_pool, &request).await?
        {
            let llm_profile: LLMDriftProfile =
                serde_json::from_value(profile).inspect_err(|e| {
                    error!("Failed to deserialize LLM drift profile: {:?}", e);
                })?;
            llm_profile
        } else {
            error!(
                "No LLM drift profile found for {}/{}/{}",
                task.space, task.name, task.version
            );
            return Ok(false);
        };

        match self.process_drift_record(&task, &llm_profile).await {
            Ok(result) => {
                task.score = serde_json::to_value(result).inspect_err(|e| {
                    error!("Failed to serialize score map: {:?}", e);
                })?;

                PostgresClient::update_llm_drift_record_status(
                    &self.db_pool,
                    &task,
                    Status::Processed,
                )
                .await?;
            }
            Err(e) => {
                error!("Failed to process drift record: {:?}", e);

                // Update the record status to error
                PostgresClient::update_llm_drift_record_status(
                    &self.db_pool,
                    &task,
                    Status::Failed,
                )
                .await?;
                return Err(DriftError::LLMEvaluatorError(e.to_string()));
            }
        };

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
            Ok(false) => Ok(()),
            Err(e) => {
                error!("Error processing drift record: {:?}", e);
                Ok(())
            }
        }
    }
}
