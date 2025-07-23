// Module for polling LLM drift records that are "pending" and need to be processed
use crate::error::DriftError;
use potato_head::agents::provider::traits::ResponseLogProbs;
use potato_head::{calculate_weighted_score, Score, StructuredOutput, TaskStatus, Workflow};
use scouter_types::llm::LLMDriftProfile;
use scouter_types::{LLMMetricRecord, LLMRecord};
use serde_json::Value;
use std::sync::Arc;
use std::sync::RwLock;
use tracing::{debug, error, instrument, warn};
pub struct LLMEvaluator {}

impl LLMEvaluator {
    pub fn new() -> Self {
        LLMEvaluator {}
    }

    /// Gets the final task results of the workflow.
    /// # Returns a HashMap where the keys are task IDs and the values are AgentResponse objects.
    pub fn get_final_task_results(
        workflow: Arc<RwLock<Workflow>>,
        profile: &LLMDriftProfile,
        record_uid: &str,
    ) -> Result<Vec<LLMMetricRecord>, DriftError> {
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
                // Get the task from the task list
                let Some(task) = task_list.get_task(task_id) else {
                    continue;
                };

                // Lock the task for reading
                let task_guard = task.read().unwrap();

                // Only process completed tasks with a result
                let (TaskStatus::Completed, Some(result)) =
                    (&task_guard.status, &task_guard.result)
                else {
                    continue;
                };

                let task_id = task_guard.id.clone();

                // Content should be returned as a json string
                let content = match result.content() {
                    Some(c) => c,
                    None => {
                        warn!("Task result content is empty for task ID: {}", task_id);
                        continue;
                    }
                };

                // Validate the content as a Score object
                let score = Score::model_validate_json_str(&content).inspect_err(|e| {
                    error!("Failed to validate score: {:?}", e);
                })?;

                // Check for log_probs in the result
                let log_probs: Vec<ResponseLogProbs> = result.log_probs();

                // Calculate weighted score if log_probs is not empty
                // Default to score if no log_probs are present or if calculation returns None
                let value = if !log_probs.is_empty() {
                    match calculate_weighted_score(&log_probs)? {
                        Some(weighted) => weighted,
                        None => score.score as f64,
                    }
                } else {
                    score.score as f64
                };

                // Create the LLMMetricRecord
                let record = LLMMetricRecord {
                    record_uid: record_uid.to_string(),
                    created_at: chrono::Utc::now(),
                    space: profile.config.space.clone(),
                    name: profile.config.name.clone(),
                    version: profile.config.version.clone(),
                    metric: task_id,
                    value,
                };

                final_results.push(record);
            }
        }

        Ok(final_results)
    }

    #[instrument(skip_all)]
    pub async fn process_drift_record(
        record: &LLMRecord,
        profile: &LLMDriftProfile,
    ) -> Result<Vec<LLMMetricRecord>, DriftError> {
        debug!("Processing workflow");

        let mut context = record.context.clone();
        let merged_context = match &mut context {
            Value::Object(ref mut map) => {
                map.insert("input".to_string(), record.input.clone());
                map.insert("response".to_string(), record.response.clone());
                debug!("Successfully merged input and response into context");
                context
            }
            _ => {
                error!("Context is not a JSON object");
                return Err(DriftError::InvalidContextFormat);
            }
        };

        let workflow_result = profile.workflow.run(Some(merged_context)).await?;
        Self::get_final_task_results(workflow_result, profile, &record.uid)
    }
}

impl Default for LLMEvaluator {
    fn default() -> Self {
        Self::new()
    }
}
