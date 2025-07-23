// Module for polling LLM drift records that are "pending" and need to be processed
use crate::error::DriftError;
use crate::llm::evaluator::LLMEvaluator;
use scouter_sql::sql::traits::{LLMDriftSqlLogic, ProfileSqlLogic};
use scouter_sql::PostgresClient;
use scouter_types::llm::LLMDriftProfile;
use scouter_types::{DriftType, GetProfileRequest, LLMRecord, Status};
use sqlx::{Pool, Postgres};
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
    ) -> Result<bool, DriftError> {
        debug!("Processing workflow");

        match LLMEvaluator::process_drift_record(record, profile).await {
            Ok(metrics) => {
                PostgresClient::insert_llm_metric_values_batch(&self.db_pool, &metrics)
                    .await
                    .inspect_err(|e| {
                        error!("Failed to insert LLM metric values: {:?}", e);
                    })?;
            }
            Err(e) => {
                error!("Failed to process drift record: {:?}", e);
                return Ok(false);
            }
        };

        Ok(true)
    }

    #[instrument(skip_all)]
    pub async fn do_poll(&mut self) -> Result<bool, DriftError> {
        // Get task from the database (query uses skip lock to pull task and update to processing)
        let task = PostgresClient::get_pending_llm_drift_record(&self.db_pool).await?;

        let Some(task) = task else {
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

        let result = self.process_drift_record(&task, &llm_profile).await?;

        // result will be false if the workflow execution failed
        // in that case, we should update the task status to Failed
        if !result {
            // Update the task status to Failed
            PostgresClient::update_llm_drift_record_status(&self.db_pool, &task, Status::Failed)
                .await
                .inspect_err(|e| {
                    error!(
                        "Failed to update LLM drift record status to Failed: {:?}",
                        e
                    );
                })?;
            return Ok(false);
        }

        // Update the run dates while still holding the lock
        PostgresClient::update_llm_drift_record_status(&self.db_pool, &task, Status::Processed)
            .await?;

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
