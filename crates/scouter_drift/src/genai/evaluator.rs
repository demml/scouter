// Module for polling GenAI drift records that are "pending" and need to be processed
use crate::error::DriftError;
use scouter_evaluate::tasks::traits::EvaluateTaskMut;
use scouter_types::genai::GenAIEvalSet;
use scouter_types::genai::{traits::ProfileExt, EvaluationContext, GenAIEvalProfile};
use scouter_types::GenAITaskRecord;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::sync::Mutex;
use tokio::task::JoinSet;
use tracing::{debug, error, instrument, warn};

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
    pub async fn process_event_record(
        record: &GenAITaskRecord,
        profile: Arc<Mutex<GenAIEvalProfile>>,
    ) -> Result<GenAIEvalSet, DriftError> {
        let results = Self::execute_tasks(record, &profile).await?;

        // convert to MetricRecords
        Ok(results)
    }

    /// Execute all tasks defined in the profile for the given record
    #[instrument(skip_all)]
    pub async fn execute_tasks(
        record: &GenAITaskRecord,
        profile: &Arc<Mutex<GenAIEvalProfile>>,
    ) -> Result<GenAIEvalSet, DriftError> {
        let begin = chrono::Utc::now();
        let eval_context = Arc::new(RwLock::new(EvaluationContext {
            context: record.context.clone(),
            task_results: HashMap::new(),
        }));

        let execution_plan = profile.lock().await.get_execution_plan()?;
        let has_llm_tasks = profile.lock().await.has_llm_tasks();
        let workflow_handle = if has_llm_tasks {
            let workflow_clone = {
                let guard = profile.lock().await;
                guard
                    .workflow
                    .as_ref()
                    .ok_or(DriftError::MissingWorkflow)?
                    .clone()
            };
            let eval_context_clone = eval_context.clone();

            // This will become an arc later within the workflow
            let cloned_ctx = record.context.clone();

            Some(tokio::spawn(async move {
                let result = workflow_clone.run(Some(cloned_ctx)).await?;

                let read_result = result
                    .read()
                    .map_err(|_| DriftError::ReadLockAcquireError)?;
                let task_list = &read_result.task_list;
                let workflow_results: HashMap<String, Value> = task_list.get_task_responses()?;

                // Populate task_results in context
                let mut context_guard = eval_context_clone
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
            Self::execute_level_concurrent(level, profile, &eval_context).await?;
        }
        let end = chrono::Utc::now();
        let duration_ms = (end - begin).num_milliseconds();

        let eval_set = profile
            .lock()
            .await
            .build_eval_set_from_tasks(record, duration_ms);

        Ok(eval_set)
    }

    async fn execute_level_concurrent(
        task_ids: &[String],
        profile: &Arc<Mutex<GenAIEvalProfile>>,
        context: &Arc<RwLock<EvaluationContext>>,
    ) -> Result<(), DriftError> {
        let mut join_set = JoinSet::new();

        for task_id in task_ids {
            let task_id = task_id.clone();
            let context = context.clone();
            let profile = profile.clone();

            // Spawn concurrent task execution
            join_set
                .spawn(async move { Self::execute_single_task(&task_id, profile, context).await });
        }

        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(Ok(())) => {
                    continue;
                }
                Ok(Err(e)) => {
                    error!("Error executing task: {:?}", e);
                    return Err(e);
                }
                Err(e) => {
                    error!("Join error: {:?}", e);
                    return Err(DriftError::GenAIEvaluatorError(format!(
                        "Join error: {}",
                        e.to_string()
                    )));
                }
            }
        }

        Ok(())
    }

    /// Executes a single task by ID, updating the profile and context as needed
    /// Results are appended to Task struct internally
    /// This allows us to keep track of results without copying/cloning internal state
    #[instrument(skip_all)]
    async fn execute_single_task(
        task_id: &str,
        profile: Arc<Mutex<GenAIEvalProfile>>,
        context: Arc<RwLock<EvaluationContext>>,
    ) -> Result<(), DriftError> {
        if let Some(mut task) = profile.lock().await.get_task_by_id_mut(task_id) {
            debug!("Executing assertion: {}", task_id);

            let context = context
                .read()
                .map_err(|_| DriftError::ReadLockAcquireError)?;

            // this will execute the task and update its internal result state
            return Ok(task.evaluate_task_mut(&context)?);
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
