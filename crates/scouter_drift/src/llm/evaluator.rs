// Module for polling LLM drift records that are "pending" and need to be processed
use crate::error::DriftError;
use potato_head::ResponseLogProbs;
use potato_head::{calculate_weighted_score, Score, StructuredOutput, TaskStatus, Workflow};
use scouter_types::llm::LLMDriftProfile;
use scouter_types::{LLMMetricRecord, LLMTaskRecord};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use tracing::{debug, error, instrument, warn};

pub type LLMEvalResult = (Vec<LLMMetricRecord>, HashMap<String, Score>, Option<i32>); // Vec<LLMMetricRecord>, ScoreMap, WorkflowDuration

pub struct LLMEvaluator {}

impl LLMEvaluator {
    pub fn new() -> Self {
        LLMEvaluator {}
    }

    /// Gets the final task results of the workflow.
    /// # Returns a HashMap where the keys are task IDs and the values are AgentResponse objects.
    /// # Arguments
    /// * `workflow` - The workflow to get the final task results from.
    /// * `profile` - The LLM drift profile.
    /// * `uid` - The unique identifier for the drift record.
    pub fn get_final_task_results(
        workflow: Arc<RwLock<Workflow>>,
        profile: &LLMDriftProfile,
        uid: &str,
    ) -> Result<LLMEvalResult, DriftError> {
        let workflow = workflow.read().unwrap();
        let task_list = &workflow.task_list;
        let execution_plan = workflow.execution_plan()?;

        let max_step = execution_plan.keys().max().copied().unwrap_or(0);

        if max_step == 0 {
            return Ok((Vec::new(), HashMap::new(), None));
        }

        let mut final_results = Vec::new();
        let mut score_map: HashMap<String, Score> = HashMap::new();
        let workflow_duration = workflow.total_duration();

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
                    entity_uid: profile.config.uid.clone(),
                    uid: uid.to_string(),
                    created_at: chrono::Utc::now(),
                    metric: task_id.clone(),
                    value,
                };

                // Add the score to the score map
                score_map.insert(task_id, score);
                final_results.push(record);
            }
        }

        Ok((final_results, score_map, Some(workflow_duration)))
    }

    #[instrument(skip_all)]
    pub async fn process_drift_record(
        record: &LLMTaskRecord,
        profile: &LLMDriftProfile,
    ) -> Result<LLMEvalResult, DriftError> {
        debug!("Processing workflow");

        let workflow_result = profile.workflow.run(Some(record.context.clone())).await?;
        Self::get_final_task_results(workflow_result, profile, &record.uid)
    }
}

impl Default for LLMEvaluator {
    fn default() -> Self {
        Self::new()
    }
}
