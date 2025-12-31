// Module for polling GenAI drift records that are "pending" and need to be processed
use crate::error::DriftError;
use potato_head::prompt_types::Score;
use potato_head::{calculate_weighted_score, StructuredOutput, TaskStatus, Workflow};

use core::task;
use scouter_types::genai::{AssertionResult, GenAIEvalProfile};
use scouter_types::{genai::EvaluationContext, GenAIMetricRecord, GenAITaskRecord};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::task::JoinSet;
use tracing::{debug, error, instrument, warn};

pub type GenAIEvalResult = (Vec<GenAIMetricRecord>, HashMap<String, Score>, Option<i32>); // Vec<GenAIMetricRecord>, ScoreMap, WorkflowDuration

pub struct GenAIEvaluator {}

impl GenAIEvaluator {
    pub fn new() -> Self {
        GenAIEvaluator {}
    }

    /// Gets the final task results of the workflow.
    /// # Returns a HashMap where the keys are task IDs and the values are AgentResponse objects.
    /// # Arguments
    /// * `workflow` - The workflow to get the final task results from.
    /// * `profile` - The GenAI drift profile.
    /// * `uid` - The unique identifier for the drift record.
    pub fn get_final_task_results(
        workflow: Arc<RwLock<Workflow>>,
        profile: &GenAIEvalProfile,
        uid: &str,
    ) -> Result<GenAIEvalResult, DriftError> {
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

                let content = result.response.extract_structured_data();

                if content.is_none() {
                    warn!("Task result content is empty for task ID: {}", task_id);
                    continue;
                };

                // TODO:
                // Validate the content as a Score object
                //let score = Score::model_validate_json_str(&content).inspect_err(|e| {
                //    error!("Failed to validate score: {:?}", e);
                //})?;

                // Check for log_probs in the result
                //let log_probs = result.log_probs();
                //
                //// Calculate weighted score if log_probs is not empty
                //// Default to score if no log_probs are present or if calculation returns None
                //let value = if !log_probs.is_empty() {
                //    match calculate_weighted_score(&log_probs)? {
                //        Some(weighted) => weighted,
                //        None => score.score as f64,
                //    }
                //} else {
                //    score.score as f64
                //};

                // Create the GenAIMetricRecord
                let record = GenAIMetricRecord {
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

    /// Process a single GenAI drift record
    /// Flow:
    /// 1. If workflow is present, execute workflow in background
    /// 2. Get results for each task in the workflow
    /// 3. Feel workflow task results into llm judge tasks and execute
    /// 4. For each field assertion, execute and collect results
    /// 5. Return all metric records and scores
    #[instrument(skip_all)]
    pub async fn process_drift_record(
        record: &GenAITaskRecord,
        profile: Arc<GenAIEvalProfile>,
    ) -> Result<GenAIEvalResult, DriftError> {
        let mut eval_context = Arc::new(RwLock::new(EvaluationContext {
            context: record.context.clone(),
            task_results: HashMap::new(),
        }));

        let execution_plan = profile.get_execution_plan()?;

        let workflow_handle = if profile.has_llm_tasks() {
            let workflow = profile
                .workflow
                .as_ref()
                .ok_or(DriftError::MissingWorkflow)?;

            let context = record.context.clone();
            let workflow_clone = workflow.clone();

            Some(tokio::spawn(async move {
                let result = workflow_clone.run(Some(context)).await?;
                let task_list = result
                    .read()
                    .map_err(|_| DriftError::ReadLockAcquireError)?;
                let workflow_results: HashMap<String, Value> = task_list.get_task_responses()?;

                // Populate task_results in context
                let mut context_guard = eval_context
                    .write()
                    .map_err(|_| DriftError::WriteLockAcquireError)?;
                for (task_id, response) in workflow_results {
                    context_guard.task_results.insert(task_id, response);
                }

                Ok::<_, DriftError>(())
            }))
        } else {
            None
        };

        // wait for workflow to complete if it was started before proceeding
        // We cant execute llm judge tasks until the workflow is done
        if let Some(handle) = workflow_handle {
            handle.await.map_err(|e| {
                DriftError::GenAIEvaluatorError(format!(
                    "Workflow task join error: {}",
                    e.to_string()
                ))
            })??;
        }

        // execute all assertion tasks concurrently per level
        // Execute tasks level by level (respecting dependencies)
        for (level_idx, level) in execution_plan.iter().enumerate() {
            debug!("Executing level {} with {} tasks", level_idx, level.len());
            let level_results =
                Self::execute_level_concurrent(level, profile, &eval_context, &record.uid).await?;
        }

        Ok((all_results, score_map, workflow_duration))
    }

    async fn execute_level_concurrent(
        task_ids: &[String],
        profile: Arc<GenAIEvalProfile>,
        context: &Arc<RwLock<EvaluationContext>>,
        record_uid: &str,
    ) -> Result<Vec<AssertionResult>, DriftError> {
        let mut join_set = JoinSet::new();

        for task_id in task_ids {
            // Clone data for async task
            let task_id = task_id.clone();
            let context = context.clone();
            let profile = profile.clone();

            // Spawn concurrent task execution
            join_set.spawn(
                async move { Self::execute_single_task(&task_id, &profile, &context).await },
            );
        }

        // Collect results
        let mut results = Vec::new();
        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(Ok(Some(assertion_result))) => results.push(assertion_result),
                Ok(Ok(None)) => continue, // LLM task, handled by workflow
                Ok(Err(e)) => {
                    error!("Task execution failed: {:?}", e);
                    return Err(e);
                }
                Err(e) => {
                    error!("Task join failed: {:?}", e);
                    return Err(DriftError::TaskExecutionError(e.to_string()));
                }
            }
        }

        Ok(results)
    }

    async fn execute_single_task(
        task_id: &str,
        profile: Arc<GenAIEvalProfile>,
        context: Arc<RwLock<EvaluationContext>>,
    ) -> Result<Option<AssertionResult>, DriftError> {
        // Check if it's an assertion
        if let Some(task) = profile.get_assertion_by_id(task_id) {
            debug!("Executing assertion: {}", task_id);
            // if task depends on other tasks, ensure their results are in context
            let context = context
                .read()
                .map_err(|_| DriftError::ReadLockAcquireError)?;

            let result = task.execute(&context)?;
            return Ok(Some(result));
        }

        // LLM judge tasks are handled by the workflow
        if profile.llm_judge_tasks.iter().any(|t| t.id == task_id) {
            debug!("Skipping LLM judge task (handled by workflow): {}", task_id);
            return Ok(None);
        }

        warn!("Task not found: {}", task_id);
        Err(DriftError::TaskNotFound(task_id.to_string()))
    }
}

impl Default for GenAIEvaluator {
    fn default() -> Self {
        Self::new()
    }
}
