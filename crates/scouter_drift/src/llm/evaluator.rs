// Module for polling LLM drift records that are "pending" and need to be processed
use crate::error::DriftError;
use scouter_sql::sql::schema::LLMDriftTaskRequest;
use scouter_sql::sql::traits::{LLMDriftSqlLogic, ProfileSqlLogic};
use scouter_sql::PostgresClient;
use scouter_types::llm::LLMDriftProfile;
use scouter_types::{DriftType, GetProfileRequest, Status};
use serde_json::Value;
use sqlx::{Pool, Postgres};
use tracing::{debug, error, info, instrument};
pub struct LLMEvaluator {
    db_pool: Pool<Postgres>,
}

impl LLMEvaluator {
    pub fn new(db_pool: &Pool<Postgres>) -> Self {
        LLMEvaluator {
            db_pool: db_pool.clone(),
        }
    }

    pub async fn process_record(
        &mut self,
        task: &LLMDriftTaskRequest,
        profile: &LLMDriftProfile,
    ) -> Result<bool, DriftError> {
        debug!("Processing workflow");

        let mut context = task.context.0.clone();

        let merged_context = match &mut context {
            Value::Object(ref mut map) => {
                // Insert input if present
                if let Some(input) = &task.input {
                    map.insert("input".to_string(), Value::String(input.clone()));
                }

                // Insert response if present
                if let Some(response) = &task.response {
                    map.insert("response".to_string(), Value::String(response.clone()));
                }

                debug!("Successfully merged input and response into context");
                context
            }
            _other => {
                error!("Context is not a JSON object");
                return Err(DriftError::InvalidContextFormat);
            }
        };

        profile.workflow.run(Some(merged_context)).await?;

        Ok(true)
    }

    #[instrument(skip_all)]
    pub async fn do_llm_eval_poll(&mut self) -> Result<bool, DriftError> {
        debug!("Polling for drift tasks");

        // Get task from the database (query uses skip lock to pull task and update to processing)
        let task = PostgresClient::get_pending_llm_drift_task(&self.db_pool).await?;

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

        self.process_record(&task, &llm_profile).await?;

        // Update the run dates while still holding the lock
        PostgresClient::update_llm_drift_task_status(&self.db_pool, &task, Status::Processed)
            .await?;

        Ok(true)
    }
}
