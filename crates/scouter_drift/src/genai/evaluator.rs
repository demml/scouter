use crate::error::DriftError;
use scouter_evaluate::tasks::traits::EvaluateTaskMut;
use scouter_types::genai::traits::{ProfileExt, TaskRefMut};
use scouter_types::genai::{AssertionResult, EvaluationContext, GenAIEvalProfile, GenAIEvalSet};
use scouter_types::GenAIEventRecord;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use tracing::{debug, instrument, warn};

/// Stores task execution results separate from the profile
#[derive(Debug, Clone)]
struct TaskResultStore {
    /// Maps task_id -> task result (for dependency resolution)
    results: Arc<RwLock<HashMap<String, AssertionResult>>>,
}

impl TaskResultStore {
    fn new() -> Self {
        Self {
            results: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn store(&self, task_id: String, result: AssertionResult) {
        self.results.write().await.insert(task_id, result);
    }

    async fn get(&self, task_id: &str) -> Option<AssertionResult> {
        self.results.read().await.get(task_id).cloned()
    }

    async fn get_all(&self) -> HashMap<String, AssertionResult> {
        self.results.read().await.clone()
    }

    /// Build context with dependency results
    async fn build_scoped_context(&self, base_context: &Value, depends_on: &[String]) -> Value {
        if depends_on.is_empty() {
            return base_context.clone();
        }

        let results_guard = self.results.read().await;
        let mut scoped_context = Self::value_to_map(base_context);

        for dep_id in depends_on {
            if let Some(dep_result) = results_guard.get(dep_id) {
                // Inject the "actual" value from the result
                scoped_context.insert(dep_id.clone(), dep_result.actual.clone());
            } else {
                warn!("Task dependency '{}' not found in results", dep_id);
            }
        }

        Value::Object(scoped_context)
    }

    fn value_to_map(value: &Value) -> serde_json::Map<String, Value> {
        match value {
            Value::Object(map) => map.clone(),
            other => {
                let mut map = serde_json::Map::new();
                map.insert("context".to_string(), other.clone());
                map
            }
        }
    }
}

/// Manages concurrent execution of GenAI evaluation tasks
pub struct GenAIEvaluator {
    /// Shared base context from the event record (immutable)
    base_context: Arc<Value>,
    /// Stores all task results for dependency resolution
    result_store: TaskResultStore,
}

impl GenAIEvaluator {
    pub fn new(base_context: Value) -> Self {
        Self {
            base_context: Arc::new(base_context),
            result_store: TaskResultStore::new(),
        }
    }

    /// Process a GenAI event record through the evaluation pipeline
    #[instrument(skip_all, fields(record_uid = %record.uid))]
    pub async fn process_event_record(
        record: &GenAIEventRecord,
        profile: Arc<tokio::sync::Mutex<GenAIEvalProfile>>,
    ) -> Result<GenAIEvalSet, DriftError> {
        let evaluator = Self::new(record.context.clone());
        evaluator.execute_tasks(record, profile).await
    }

    /// Execute all tasks level-by-level according to the execution plan
    #[instrument(skip_all)]
    async fn execute_tasks(
        &self,
        record: &GenAIEventRecord,
        profile: Arc<GenAIEvalProfile>,
    ) -> Result<GenAIEvalSet, DriftError> {
        let begin = chrono::Utc::now();
        let execution_plan = profile.get_execution_plan()?;

        for (level_idx, level_tasks) in execution_plan.iter().enumerate() {
            debug!(
                "Executing level {} with {} tasks",
                level_idx,
                level_tasks.len()
            );
            self.execute_level(&level_tasks, &profile, level_idx)
                .await?;
        }

        let duration_ms = (chrono::Utc::now() - begin).num_milliseconds();

        // Reconcile results back to profile at the end
        let eval_set = self.build_eval_set(record, &profile, duration_ms).await;

        Ok(eval_set)
    }

    /// Execute all tasks in a single level (assertions and LLM judges)
    async fn execute_level(
        &self,
        task_ids: &[String],
        profile: &Arc<GenAIEvalProfile>,
        level_idx: usize,
    ) -> Result<(), DriftError> {
        let (assertion_ids, llm_judge_ids) = self.partition_tasks(task_ids, profile).await;

        // Execute assertions concurrently
        if !assertion_ids.is_empty() {
            self.execute_assertions(&assertion_ids, profile).await?;
        }

        // Execute LLM judges via workflow
        if !llm_judge_ids.is_empty() {
            self.execute_llm_judges(&llm_judge_ids, profile, level_idx)
                .await?;
        }

        Ok(())
    }

    /// Partition task IDs by type (assertions vs LLM judges)
    async fn partition_tasks(
        &self,
        task_ids: &[String],
        profile: &Arc<tokio::sync::Mutex<GenAIEvalProfile>>,
    ) -> (Vec<String>, Vec<String>) {
        let guard = profile.lock().await;
        task_ids
            .iter()
            .partition(|id| guard.get_assertion_by_id(id).is_some())
    }

    /// Execute assertion tasks concurrently
    async fn execute_assertions(
        &self,
        task_ids: &[String],
        profile: &Arc<tokio::sync::Mutex<GenAIEvalProfile>>,
    ) -> Result<(), DriftError> {
        let mut join_set = JoinSet::new();

        for task_id in task_ids {
            let task_id = task_id.clone();
            let profile = profile.clone();
            let base_context = self.base_context.clone();
            let task_results = self.task_results.clone();

            join_set.spawn(async move {
                Self::execute_assertion_task(&task_id, profile, base_context, task_results).await
            });
        }

        // Collect all results
        while let Some(result) = join_set.join_next().await {
            result.map_err(|e| {
                DriftError::GenAIEvaluatorError(format!("Task join error: {}", e))
            })??;
        }

        Ok(())
    }

    /// Execute a single assertion task with scoped context
    #[instrument(skip_all, fields(task_id = %task_id))]
    async fn execute_assertion_task(
        task_id: &str,
        profile: Arc<tokio::sync::Mutex<GenAIEvalProfile>>,
        base_context: Arc<Value>,
        task_results: Arc<RwLock<HashMap<String, AssertionResult>>>,
    ) -> Result<(), DriftError> {
        // Build scoped context with dependencies
        let scoped_context = {
            let profile_guard = profile.lock().await;
            let task = profile_guard
                .get_task_by_id(task_id)
                .ok_or_else(|| DriftError::TaskNotFound(task_id.to_string()))?;

            let depends_on = task.depends_on().to_vec();
            let results_guard = task_results.read().await;
            let context = Self::build_scoped_context(&base_context, &depends_on, &results_guard);

            (context, depends_on)
        };

        // Execute task and update result
        {
            let mut profile_guard = profile.lock().await;
            let mut task = profile_guard
                .get_task_by_id_mut(task_id)
                .ok_or_else(|| DriftError::TaskNotFound(task_id.to_string()))?;

            let eval_context = EvaluationContext {
                context: scoped_context,
                task_results: HashMap::new(),
            };

            task.evaluate_task_mut(&eval_context)?;
        }

        // Store result for downstream dependencies
        Self::store_task_result(task_id, &profile, &task_results).await?;

        Ok(())
    }

    /// Execute LLM judge tasks via workflow
    async fn execute_llm_judges(
        &self,
        llm_judge_ids: &[String],
        profile: &Arc<tokio::sync::Mutex<GenAIEvalProfile>>,
        level_idx: usize,
    ) -> Result<(), DriftError> {
        // Build workflow from LLM judges
        let workflow = {
            let guard = profile.lock().await;
            let judges: Vec<_> = llm_judge_ids
                .iter()
                .filter_map(|id| guard.get_llm_judge_by_id(id))
                .cloned()
                .collect();

            GenAIEvalProfile::build_workflow_from_judges(&judges).await?
        };

        // Build merged context for all LLM judges in this level
        let workflow_context = self.build_workflow_context(llm_judge_ids, profile).await?;

        // Execute workflow
        debug!(
            "Executing workflow for level {} with {} LLM judges",
            level_idx,
            llm_judge_ids.len()
        );
        let result = workflow.run(Some(workflow_context)).await?;

        // Process workflow results
        let workflow_results: HashMap<String, Value> = {
            let read_result = result
                .read()
                .map_err(|_| DriftError::ReadLockAcquireError)?;
            read_result.task_list.get_task_responses()?
        };

        // Update tasks and store results
        self.process_workflow_results(workflow_results, profile)
            .await?;

        Ok(())
    }

    /// Build merged context for workflow execution
    async fn build_workflow_context(
        &self,
        llm_judge_ids: &[String],
        profile: &Arc<tokio::sync::Mutex<GenAIEvalProfile>>,
    ) -> Result<Value, DriftError> {
        let guard = profile.lock().await;
        let results_guard = self.task_results.read().await;

        // Collect all unique dependencies first
        let mut all_deps = std::collections::HashSet::new();
        for judge_id in llm_judge_ids {
            if let Some(judge) = guard.get_llm_judge_by_id(judge_id) {
                all_deps.extend(judge.depends_on.iter().cloned());
            }
        }

        // If no dependencies, return base context as-is (any type)
        if all_deps.is_empty() {
            return Ok((*self.base_context).clone());
        }

        // With dependencies, we need to create a context object
        let mut merged_context = match self.base_context.as_ref() {
            Value::Object(map) => map.clone(),
            other => {
                let mut map = serde_json::Map::new();
                map.insert("context".to_string(), other.clone());
                map
            }
        };

        // Inject dependency results (extract "actual" field from serialized AssertionResult)
        for dep_id in all_deps {
            if let Some(dep_result) = results_guard.get(&dep_id) {
                // Extract "actual" field from the serialized result
                let value_to_inject = if let Some(actual) = dep_result.get("actual") {
                    actual.clone()
                } else {
                    dep_result.clone()
                };
                merged_context.insert(dep_id, value_to_inject);
            }
        }

        Ok(Value::Object(merged_context))
    }

    /// Process workflow results and update profile tasks
    async fn process_workflow_results(
        &self,
        workflow_results: HashMap<String, Value>,
        profile: &Arc<tokio::sync::Mutex<GenAIEvalProfile>>,
    ) -> Result<(), DriftError> {
        let mut profile_guard = profile.lock().await;
        let mut results_guard = self.task_results.write().await;

        for (task_id, response) in workflow_results {
            // Update LLM judge task with result
            if let Some(mut task) = profile_guard.get_task_by_id_mut(&task_id) {
                Self::update_llm_judge_result(&mut task, &response)?;
            }

            // Store result for downstream tasks
            results_guard.insert(task_id, response);
        }

        Ok(())
    }

    /// Build scoped context for a task based on its dependencies
    fn build_scoped_context(
        base_context: &Value,
        depends_on: &[String],
        task_results: &HashMap<String, AssertionResult>,
    ) -> Value {
        if depends_on.is_empty() {
            return base_context.clone();
        }

        let mut scoped_context = Self::value_to_map(base_context);

        for dep_id in depends_on {
            if let Some(dep_result) = task_results.get(dep_id) {
                scoped_context.insert(dep_id.clone(), dep_result.actual.clone());
            } else {
                warn!("Task dependency '{}' not found in results", dep_id);
            }
        }

        Value::Object(scoped_context)
    }

    /// Convert Value to serde_json::Map, wrapping non-objects
    fn value_to_map(value: &Value) -> serde_json::Map<String, Value> {
        match value {
            Value::Object(map) => map.clone(),
            other => {
                let mut map = serde_json::Map::new();
                map.insert("context".to_string(), other.clone());
                map
            }
        }
    }

    /// Store a task's result in the shared results map
    async fn store_task_result(
        task_id: &str,
        profile: &Arc<tokio::sync::Mutex<GenAIEvalProfile>>,
        task_results: &Arc<RwLock<HashMap<String, Value>>>,
    ) -> Result<(), DriftError> {
        let profile_guard = profile.lock().await;
        if let Some(task) = profile_guard.get_task_by_id(task_id) {
            if let Some(result) = task.get_result() {
                let result_value = serde_json::to_value(result).map_err(|e| {
                    DriftError::GenAIEvaluatorError(format!("Failed to serialize result: {}", e))
                })?;

                task_results
                    .write()
                    .await
                    .insert(task_id.to_string(), result_value);
            }
        }
        Ok(())
    }

    /// Update LLM judge task with workflow result
    fn update_llm_judge_result(
        task: &mut TaskRefMut<'_>,
        response: &Value,
    ) -> Result<(), DriftError> {
        match task {
            TaskRefMut::LLMJudge(judge) => {
                let actual_value = response.clone();
                let passed = judge
                    .operator
                    .compare(&actual_value, &judge.expected_value)?;

                judge.result = Some(AssertionResult {
                    passed,
                    actual: actual_value,
                    message: if passed {
                        "LLM judge evaluation passed".to_string()
                    } else {
                        format!(
                            "Expected {:?} {} {:?}",
                            judge.expected_value, judge.operator, actual_value
                        )
                    },
                });

                Ok(())
            }
            _ => Err(DriftError::GenAIEvaluatorError(
                "Expected LLMJudge task".to_string(),
            )),
        }
    }
}
