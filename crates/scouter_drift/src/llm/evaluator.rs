// Module for polling LLM drift records that are "pending" and need to be processed
use crate::error::DriftError;
use potato_head::Score;
use potato_head::StructuredOutput;
use potato_head::TaskStatus;
use potato_head::Workflow;
use scouter_sql::sql::schema::LLMDriftTaskRequest;
use scouter_sql::sql::traits::{LLMDriftSqlLogic, ProfileSqlLogic};
use scouter_sql::PostgresClient;
use scouter_types::llm::LLMDriftProfile;
use scouter_types::{DriftType, GetProfileRequest, LLMMetricServerRecord, Status};
use serde_json::Value;
use sqlx::{Pool, Postgres};
use std::sync::Arc;
use std::sync::RwLock;
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

    /// Gets the final task results of the workflow.
    /// # Returns a HashMap where the keys are task IDs and the values are AgentResponse objects.
    pub fn get_final_task_results(
        &self,
        workflow: Arc<RwLock<Workflow>>,
        profile: &LLMDriftProfile,
    ) -> Result<Vec<LLMMetricServerRecord>, DriftError> {
        let workflow = workflow.read().unwrap();
        let task_list = &workflow.task_list;
        let execution_plan = workflow.execution_plan()?;

        let max_step = execution_plan.keys().max().copied().unwrap_or(0);

        if max_step == 0 {
            return Ok(Vec::new());
        }

        let mut final_results = Vec::new();

        if let Some(final_task_ids) = execution_plan.get(&max_step) {
            for task_id in final_task_ids {
                if let Some(task) = task_list.get_task(task_id) {
                    let task_guard = task.read().unwrap();
                    if task_guard.status == TaskStatus::Completed {
                        if let Some(result) = &task_guard.result {
                            let task_id = task_guard.id.clone();
                            let content = result.content();
                            let score =
                                Score::model_validate_json_value(&content).inspect_err(|e| {
                                    error!("Failed to validate score: {:?}", e);
                                })?;

                            let record = LLMMetricServerRecord {
                                created_at: chrono::Utc::now(),
                                space: profile.config.space.clone(),
                                name: profile.config.name.clone(),
                                version: profile.config.version.clone(),
                                metric: task_id,
                                value: (score.score as f64),
                            };

                            final_results.push(record);
                        }
                    }
                }
            }
        }

        Ok(final_results)
    }

    #[instrument(skip_all)]
    pub async fn process_drift_record(
        &mut self,
        task: &LLMDriftTaskRequest,
        profile: &LLMDriftProfile,
    ) -> Result<bool, DriftError> {
        debug!("Processing workflow");

        let mut context = task.context.clone();
        let merged_context = match &mut context {
            Value::Object(ref mut map) => {
                // Insert input if not empty
                map.insert("input".to_string(), task.input.clone());
                map.insert("response".to_string(), task.response.clone());
                debug!("Successfully merged input and response into context");
                context
            }
            _other => {
                error!("Context is not a JSON object");
                return Err(DriftError::InvalidContextFormat);
            }
        };

        let workflow_result = profile
            .workflow
            .run(Some(merged_context))
            .await
            .inspect_err(|e| {
                error!("Failed to run workflow: {:?}", e);
            })?;

        let final_results = self
            .get_final_task_results(workflow_result, profile)
            .inspect_err(|e| {
                error!("Failed to get final task results: {:?}", e);
            })?;

        PostgresClient::insert_llm_metric_values_batch(&self.db_pool, &final_results)
            .await
            .inspect_err(|e| {
                error!("Failed to insert LLM metric values: {:?}", e);
            })?;

        Ok(true)
    }

    #[instrument(skip_all)]
    pub async fn do_poll(&mut self) -> Result<bool, DriftError> {
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

        self.process_drift_record(&task, &llm_profile).await?;

        // Update the run dates while still holding the lock
        PostgresClient::update_llm_drift_task_status(&self.db_pool, &task, Status::Processed)
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
