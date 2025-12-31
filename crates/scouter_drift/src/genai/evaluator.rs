// Module for polling GenAI drift records that are "pending" and need to be processed
use crate::error::DriftError;
use potato_head::prompt_types::Score;
use potato_head::{TaskStatus, Workflow};
use scouter_evaluate::tasks::traits::EvaluationTask;
use scouter_types::genai::{
    traits::{ProfileExt, TaskAccessor, TaskRef},
    AssertionResult, EvaluationContext, GenAIEvalProfile,
};
use scouter_types::{GenAIMetricRecord, GenAITaskRecord};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::task::JoinSet;
use tracing::{debug, error, instrument, warn};
pub type GenAIEvalResult = (Vec<GenAIMetricRecord>, HashMap<String, Score>, Option<i32>); // Vec<GenAIMetricRecord>, ScoreMap, WorkflowDuration

struct AssertionResults {
    pub results: Vec<AssertionResult>,
}

impl AssertionResults {
    fn new() -> Self {
        AssertionResults {
            results: Vec::new(),
        }
    }

    // consumes and allows adding multiple results at once
    fn add_results(&mut self, results: Vec<AssertionResult>) {
        self.results.extend(results);
    }
}

pub struct GenAIEvaluator {}

impl GenAIEvaluator {
    pub fn new() -> Self {
        GenAIEvaluator {}
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
    ) -> Result<AssertionResults, DriftError> {
        let mut eval_context = Arc::new(RwLock::new(EvaluationContext {
            context: record.context.clone(),
            task_results: HashMap::new(),
        }));

        let mut assertions_results = AssertionResults::new();

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
                Self::execute_level_concurrent(level, profile, &eval_context).await?;
            assertions_results.add_results(level_results);
        }

        Ok(assertions_results)
    }

    async fn execute_level_concurrent(
        task_ids: &[String],
        profile: Arc<GenAIEvalProfile>,
        context: &Arc<RwLock<EvaluationContext>>,
    ) -> Result<Vec<AssertionResult>, DriftError> {
        let mut join_set = JoinSet::new();

        for task_id in task_ids {
            let task_id = task_id.clone();
            let context = context.clone();
            let profile = profile.clone();

            // Spawn concurrent task execution
            join_set
                .spawn(async move { Self::execute_single_task(&task_id, profile, context).await });
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

    #[instrument(skip_all)]
    async fn execute_single_task(
        task_id: &str,
        profile: Arc<GenAIEvalProfile>,
        context: Arc<RwLock<EvaluationContext>>,
    ) -> Result<Option<AssertionResult>, DriftError> {
        if let Some(task) = profile.get_task_by_id(task_id) {
            debug!("Executing assertion: {}", task_id);

            let context = context
                .read()
                .map_err(|_| DriftError::ReadLockAcquireError)?;

            let result = match task {
                TaskRef::Assertion(assertion) => assertion.execute(&context)?,
                TaskRef::LLMJudge(judge) => judge.execute(&context)?,
            };
            return Ok(Some(result));
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
