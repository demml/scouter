use crate::error::EvaluationError;
use crate::evaluate::store::{AssertionResultStore, LLMResponseStore, TaskRegistry, TaskType};
use crate::tasks::traits::EvaluationTask;
use scouter_types::genai::traits::ProfileExt;
use scouter_types::genai::{AssertionResult, GenAIEvalProfile, GenAIEvalSet};
use scouter_types::GenAIEvalRecord;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use tracing::{debug, error, instrument, warn};

/// Stores task execution results separate from the profile
#[derive(Debug, Clone)]
struct TaskResultStore {
    /// Maps task_id -> task result (for dependency resolution)
    pub assertion_store: Arc<RwLock<AssertionResultStore>>,
    pub llm_response_store: Arc<RwLock<LLMResponseStore>>,
    pub task_registry: Arc<RwLock<TaskRegistry>>,
}

impl TaskResultStore {
    fn new() -> Self {
        Self {
            assertion_store: Arc::new(RwLock::new(AssertionResultStore::new())),
            llm_response_store: Arc::new(RwLock::new(LLMResponseStore::new())),
            task_registry: Arc::new(RwLock::new(TaskRegistry::new())),
        }
    }

    /// Register tasks from profile during initialization
    async fn register_tasks(&self, profile: &GenAIEvalProfile) {
        let mut registry = self.task_registry.write().await;

        // Register assertion tasks
        for task in &profile.assertion_tasks {
            registry.register(task.id.clone(), TaskType::Assertion);
            if !task.depends_on.is_empty() {
                registry.register_dependencies(task.id.clone(), task.depends_on.clone());
            }
        }

        // Register LLM judge tasks
        for task in &profile.llm_judge_tasks {
            registry.register(task.id.clone(), TaskType::LLMJudge);
            if !task.depends_on.is_empty() {
                registry.register_dependencies(task.id.clone(), task.depends_on.clone());
            }
        }
    }

    /// Store assertion result and register task type
    async fn store_assertion(&self, task_id: String, result: AssertionResult) {
        let mut store = self.assertion_store.write().await;
        store.store(task_id, result);
    }

    /// Store LLM response and register task type
    async fn store_llm_response(&self, task_id: String, response: serde_json::Value) {
        let mut store = self.llm_response_store.write().await;
        store.store(task_id, response);
    }

    /// Build context with dependency results
    /// If no dependencies, return base context as-is
    /// if dependencies exist, merge them and base context into new mapping
    /// Example:
    /// base_context = { "a": 1, "b": 2 }
    /// dep results = { "task1": 10, "task2": 20 }
    /// merged context = { "context": { "a": 1, "b": 2 }, "task1": 10, "task2": 20 }
    /// Build context with dependency results, routing to correct store
    async fn build_scoped_context(&self, base_context: &Value, depends_on: &[String]) -> Value {
        if depends_on.is_empty() {
            return base_context.clone();
        }

        let mut scoped_context = Self::build_context_map(base_context);
        let registry = self.task_registry.read().await;

        for dep_id in depends_on {
            match registry.get_type(dep_id) {
                Some(TaskType::Assertion) => {
                    let assertion_store = self.assertion_store.read().await;
                    if let Some(dep_result) = assertion_store.retrieve(dep_id) {
                        scoped_context.insert(dep_id.clone(), dep_result.actual.clone());
                    } else {
                        warn!("Assertion dependency '{}' not found in results", dep_id);
                    }
                }
                Some(TaskType::LLMJudge) => {
                    let llm_store = self.llm_response_store.read().await;
                    if let Some(dep_response) = llm_store.retrieve(dep_id) {
                        scoped_context.insert(dep_id.clone(), dep_response.clone());
                    } else {
                        warn!("LLM judge dependency '{}' not found in results", dep_id);
                    }
                }
                None => {
                    warn!(
                        "Task dependency '{}' not registered in task registry",
                        dep_id
                    );
                }
            }
        }

        Value::Object(scoped_context)
    }

    // for all non level-0 tasks, we build a context map of base context + dep results
    fn build_context_map(value: &Value) -> serde_json::Map<String, Value> {
        let mut map = serde_json::Map::new();

        // if base context is an object, do nothing
        // if not, wrap it under "context" key
        match value {
            Value::Object(obj) => obj.clone(),
            _ => {
                map.insert("context".to_string(), value.clone());
                map
            }
        }
    }

    /// Check if assertion task passed
    async fn check_assertion_passed(&self, task_id: &str) -> Option<bool> {
        let store = self.assertion_store.read().await;
        store.retrieve(task_id).map(|res| res.passed)
    }

    /// Check if all conditional dependencies passed for a given task
    /// Returns None if conditions haven't been evaluated yet
    /// Returns Some(true) if all conditions passed or no conditions exist
    /// Returns Some(false) if any condition failed
    async fn check_conditional_passed(&self, task_id: &str) -> Option<bool> {
        let registry = self.task_registry.read().await;
        let condition_deps = registry.get_dependencies(task_id)?;

        if condition_deps.is_empty() {
            return Some(true);
        }

        let mut results = Vec::with_capacity(condition_deps.len());
        for dep_id in condition_deps {
            results.push(self.check_assertion_passed(dep_id).await);
        }

        if results.iter().any(|r| matches!(r, Some(false))) {
            return Some(false);
        }

        let all_missing = results.iter().all(|r| r.is_none());
        if all_missing {
            return Some(false);
        }

        let any_missing = results.iter().any(|r| r.is_none());
        if any_missing {
            return None;
        }

        Some(true)
    }

    /// Filter task IDs based on conditional dependencies
    /// Returns only tasks whose conditional dependencies have all passed
    async fn filter_conditional_tasks(&self, task_ids: &[String]) -> Vec<String> {
        let mut filtered = Vec::with_capacity(task_ids.len());

        for task_id in task_ids {
            match self.check_conditional_passed(task_id).await {
                Some(true) => filtered.push(task_id.clone()),
                Some(false) => {
                    debug!(
                        "Skipping task '{}' due to failed conditional dependency",
                        task_id
                    );
                }
                None => {
                    warn!(
                        "Task '{}' has unevaluated conditional dependencies",
                        task_id
                    );
                }
            }
        }

        filtered
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
    async fn new(base_context: Value, profile: &Arc<GenAIEvalProfile>) -> Self {
        let result_store = TaskResultStore::new();
        result_store.register_tasks(profile).await;

        Self {
            base_context: Arc::new(base_context),
            result_store,
        }
    }

    /// Process a GenAI event record through the evaluation pipeline
    #[instrument(skip_all, fields(record_uid = %record.uid))]
    pub async fn process_event_record(
        record: &GenAIEvalRecord,
        profile: Arc<GenAIEvalProfile>,
    ) -> Result<GenAIEvalSet, EvaluationError> {
        let evaluator = Self::new(record.context.clone(), &profile).await;
        evaluator.execute_tasks(record, profile).await
    }

    #[instrument(skip_all)]
    async fn execute_tasks(
        &self,
        record: &GenAIEvalRecord,
        profile: Arc<GenAIEvalProfile>,
    ) -> Result<GenAIEvalSet, EvaluationError> {
        let begin = chrono::Utc::now();

        let execution_plan = profile.get_execution_plan()?;
        self.result_store
            .task_registry
            .write()
            .await
            .set_first_level_tasks(execution_plan.first().cloned().unwrap_or_default());

        for (level_idx, level_tasks) in execution_plan.iter().enumerate() {
            debug!(
                "Executing level {} with {} tasks",
                level_idx,
                level_tasks.len()
            );
            self.execute_level(level_tasks, &profile)
                .await
                .inspect_err(|e| error!("Failed to execute level {}: {:?}", level_idx, e))?;
        }

        let end = chrono::Utc::now();
        let duration_ms = (end - begin).num_milliseconds();

        // Reconcile results back to profile at the end
        let eval_set = self.build_eval_set(record, &profile, duration_ms).await;

        Ok(eval_set)
    }

    /// Partition task IDs by type (assertions vs LLM judges)
    async fn partition_tasks(&self, task_ids: &[String]) -> (Vec<String>, Vec<String>) {
        let mut assertions = Vec::new();
        let mut judges = Vec::new();

        for id in task_ids {
            match self.result_store.task_registry.read().await.get_type(id) {
                Some(TaskType::Assertion) => assertions.push(id.clone()),
                Some(TaskType::LLMJudge) => judges.push(id.clone()),
                _ => continue,
            }
        }

        (assertions, judges)
    }

    /// Execute assertion tasks concurrently
    async fn execute_assertions(
        &self,
        task_ids: &[String],
        profile: &Arc<GenAIEvalProfile>,
    ) -> Result<(), EvaluationError> {
        let mut join_set = JoinSet::new();

        for task_id in task_ids {
            let task_id = task_id.clone();
            let profile = profile.clone();
            let base_context = self.base_context.clone();
            let result_store = self.result_store.clone();

            join_set.spawn(async move {
                Self::execute_assertion_task(&task_id, profile, base_context, result_store).await
            });
        }

        while let Some(result) = join_set.join_next().await {
            result.map_err(|e| {
                EvaluationError::GenAIEvaluatorError(format!("Task join error: {}", e))
            })??;
        }

        Ok(())
    }

    #[instrument(skip_all)]
    async fn execute_level(
        &self,
        task_ids: &[String],
        profile: &Arc<GenAIEvalProfile>,
    ) -> Result<(), EvaluationError> {
        debug!(
            "Executing level with {} tasks before conditional filtering",
            task_ids.len()
        );

        let filtered_task_ids = self.result_store.filter_conditional_tasks(task_ids).await;

        debug!(
            "{} tasks to execute after conditional filtering",
            filtered_task_ids.len()
        );

        if filtered_task_ids.is_empty() {
            debug!("No tasks to execute after conditional filtering");
            return Ok(());
        }

        let (assertion_ids, llm_judge_ids) = self.partition_tasks(&filtered_task_ids).await;

        let (assertion_result, judge_result) = tokio::join!(
            async {
                if !assertion_ids.is_empty() {
                    self.execute_assertions(&assertion_ids, profile).await
                } else {
                    Ok(())
                }
            },
            async {
                if !llm_judge_ids.is_empty() {
                    self.execute_llm_judges_for_level(&llm_judge_ids, profile)
                        .await
                        .inspect_err(|e| error!("Failed to execute LLM judge tasks: {:?}", e))
                } else {
                    Ok(())
                }
            }
        );

        assertion_result?;
        judge_result?;

        Ok(())
    }

    #[instrument(skip_all)]
    async fn execute_llm_judges_for_level(
        &self,
        judge_ids: &[String],
        profile: &Arc<GenAIEvalProfile>,
    ) -> Result<(), EvaluationError> {
        let mut join_set = JoinSet::new();

        for judge_id in judge_ids {
            let judge_id = judge_id.clone();
            let profile = profile.clone();
            let base_context = self.base_context.clone();
            let result_store = self.result_store.clone();

            join_set.spawn(async move {
                Self::execute_llm_judge_task(&judge_id, profile, base_context, result_store).await
            });
        }

        let mut results = HashMap::new();
        while let Some(task_result) = join_set.join_next().await {
            let (judge_id, response) = task_result.map_err(|e| {
                EvaluationError::GenAIEvaluatorError(format!("Task join error: {}", e))
            })??;
            results.insert(judge_id, response);
        }

        self.process_llm_judge_results(results, profile).await?;

        Ok(())
    }

    #[instrument(skip_all, fields(task_id = %task_id))]
    async fn execute_llm_judge_task(
        task_id: &str,
        profile: Arc<GenAIEvalProfile>,
        base_context: Arc<Value>,
        result_store: TaskResultStore,
    ) -> Result<(String, Value), EvaluationError> {
        let judge = profile
            .get_llm_judge_by_id(task_id)
            .ok_or_else(|| EvaluationError::TaskNotFound(task_id.to_string()))?;

        let scoped_context = result_store
            .build_scoped_context(&base_context, &judge.depends_on)
            .await;

        debug!("Executing LLM judge '{}' with scoped context", task_id);

        let workflow = profile.workflow.as_ref().ok_or_else(|| {
            EvaluationError::GenAIEvaluatorError("No workflow defined in profile".to_string())
        })?;

        let task_result = workflow.execute_task(task_id, &scoped_context).await?;

        debug!("LLM judge '{}' completed", task_id);

        Ok((task_id.to_string(), task_result))
    }

    /// Execute a single assertion task with scoped context
    /// Results are stored in the shared result store
    #[instrument(skip_all, fields(task_id = %task_id))]
    async fn execute_assertion_task(
        task_id: &str,
        profile: Arc<GenAIEvalProfile>,
        base_context: Arc<Value>,
        result_store: TaskResultStore,
    ) -> Result<(), EvaluationError> {
        let task = profile
            .get_assertion_by_id(task_id)
            .ok_or_else(|| EvaluationError::TaskNotFound(task_id.to_string()))?;

        let scoped_context = result_store
            .build_scoped_context(&base_context, &task.depends_on)
            .await;

        debug!(
            "Executing assertion task '{}' with scoped context {:?}",
            task_id, scoped_context
        );

        let result = task.execute(&scoped_context)?;

        let is_first_level = result_store
            .task_registry
            .read()
            .await
            .is_first_level_task(task_id);

        let should_store_result = if task.condition {
            result.passed
        } else {
            is_first_level || result.passed
        };

        if should_store_result {
            result_store
                .store_assertion(task_id.to_string(), result)
                .await;
        } else {
            debug!(
                "Skipping storage of {} conditional '{}' (branch not taken)",
                if is_first_level {
                    "first-level"
                } else {
                    "nested"
                },
                task_id
            );
        }

        Ok(())
    }

    /// Process LLM judge results and store them
    async fn process_llm_judge_results(
        &self,
        results: HashMap<String, Value>,
        profile: &Arc<GenAIEvalProfile>,
    ) -> Result<(), EvaluationError> {
        for (task_id, response) in results {
            // Store raw response
            self.result_store
                .store_llm_response(task_id.clone(), response.clone())
                .await;

            // Execute assertion on response
            if let Some(task) = profile.get_llm_judge_by_id(&task_id) {
                let assertion_result = task.execute(&response)?;

                self.result_store
                    .store_assertion(task_id.clone(), assertion_result)
                    .await;
            }
        }

        Ok(())
    }

    /// Build final eval set from stored results
    async fn build_eval_set(
        &self,
        record: &GenAIEvalRecord,
        profile: &Arc<GenAIEvalProfile>,
        duration_ms: i64,
    ) -> GenAIEvalSet {
        let mut passed_count = 0;
        let mut failed_count = 0;
        let mut records = Vec::new();

        // Process assertion results - only tasks that were actually stored
        for assertion in &profile.assertion_tasks {
            let assert_store = self.result_store.assertion_store.read().await;
            if let Some(result) = assert_store.retrieve(&assertion.id) {
                if result.passed {
                    passed_count += 1;
                } else {
                    failed_count += 1;
                }

                records.push(scouter_types::GenAIEvalTaskResult {
                    created_at: chrono::Utc::now(),
                    record_uid: record.uid.clone(),
                    entity_id: record.entity_id,
                    task_id: assertion.id.clone(),
                    task_type: assertion.task_type.clone(),
                    passed: result.passed,
                    value: result.to_metric_value(),
                    field_path: assertion.field_path.clone(),
                    expected: result.expected.clone(), // use result expected value because it may be templated
                    actual: result.actual.clone(),
                    message: result.message.clone(),
                    operator: assertion.operator.clone(),
                    entity_uid: String::new(),
                });
            }
        }

        // Process LLM judge results - only tasks that were actually stored
        for judge in &profile.llm_judge_tasks {
            let assert_store = self.result_store.assertion_store.read().await;
            if let Some(result) = assert_store.retrieve(&judge.id) {
                if result.passed {
                    passed_count += 1;
                } else {
                    failed_count += 1;
                }

                records.push(scouter_types::GenAIEvalTaskResult {
                    created_at: chrono::Utc::now(),
                    record_uid: record.uid.clone(),
                    entity_id: record.entity_id,
                    task_id: judge.id.clone(),
                    task_type: judge.task_type.clone(),
                    passed: result.passed,
                    value: result.to_metric_value(),
                    field_path: judge.field_path.clone(),
                    expected: judge.expected_value.clone(),
                    actual: result.actual.clone(),
                    message: result.message.clone(),
                    operator: judge.operator.clone(),
                    entity_uid: String::new(),
                });
            }
        }

        let workflow_record = scouter_types::GenAIEvalWorkflowResult {
            created_at: chrono::Utc::now(),
            entity_id: record.entity_id,
            record_uid: record.uid.clone(),
            total_tasks: (passed_count + failed_count),
            passed_tasks: passed_count,
            failed_tasks: failed_count,
            pass_rate: if passed_count + failed_count == 0 {
                0.0
            } else {
                (passed_count as f64) / ((passed_count + failed_count) as f64)
            },
            duration_ms,
            entity_uid: String::new(),
        };

        GenAIEvalSet::new(records, workflow_record)
    }
}

#[cfg(test)]
mod tests {

    use chrono::Utc;
    use potato_head::mock::{create_score_prompt, LLMTestServer};
    use scouter_types::genai::{
        AssertionTask, ComparisonOperator, GenAIAlertConfig, GenAIDriftConfig, GenAIEvalProfile,
        LLMJudgeTask,
    };
    use scouter_types::genai::{EvaluationTaskType, EvaluationTasks};
    use scouter_types::GenAIEvalRecord;
    use serde_json::Value;
    use std::sync::Arc;

    use crate::evaluate::GenAIEvaluator;

    async fn create_assert_judge_profile() -> GenAIEvalProfile {
        let prompt = create_score_prompt(Some(vec!["input".to_string()]));

        let assertion_level_1 = AssertionTask {
            id: "input_check".to_string(),
            field_path: Some("input.foo".to_string()),
            operator: ComparisonOperator::Equals,
            expected_value: Value::String("bar".to_string()),
            description: Some("Check if input.foo is bar".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let judge_task_level_1 = LLMJudgeTask::new_rs(
            "query_relevance",
            prompt.clone(),
            Value::Number(1.into()),
            Some("score".to_string()),
            ComparisonOperator::GreaterThanOrEqual,
            None,
            None,
            None,
            None,
        );

        let assert_query_score = AssertionTask {
            id: "assert_score".to_string(),
            field_path: Some("query_relevance.score".to_string()),
            operator: ComparisonOperator::IsNumeric,
            expected_value: Value::Bool(true),
            depends_on: vec!["query_relevance".to_string()],
            task_type: EvaluationTaskType::Assertion,
            description: Some("Check that score is numeric".to_string()),
            result: None,
            condition: false,
        };

        let assert_query_reason = AssertionTask {
            id: "assert_reason".to_string(),
            field_path: Some("query_relevance.reason".to_string()),
            operator: ComparisonOperator::IsString,
            expected_value: Value::Bool(true),
            depends_on: vec!["query_relevance".to_string()],
            task_type: EvaluationTaskType::Assertion,
            description: Some("Check that reason is alphabetic".to_string()),
            result: None,
            condition: false,
        };

        let tasks = EvaluationTasks::new()
            .add_task(assertion_level_1)
            .add_task(judge_task_level_1)
            .add_task(assert_query_score)
            .add_task(assert_query_reason)
            .build();

        let alert_config = GenAIAlertConfig::default();

        let drift_config =
            GenAIDriftConfig::new("scouter", "ML", "0.1.0", 1.0, alert_config, None).unwrap();

        GenAIEvalProfile::new(drift_config, tasks).await.unwrap()
    }

    async fn create_assert_profile() -> GenAIEvalProfile {
        let assert1 = AssertionTask {
            id: "input_foo_check".to_string(),
            field_path: Some("input.foo".to_string()),
            operator: ComparisonOperator::Equals,
            expected_value: Value::String("bar".to_string()),
            description: Some("Check if input.foo is bar".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let assert2 = AssertionTask {
            id: "input_bar_check".to_string(),
            field_path: Some("input.bar".to_string()),
            operator: ComparisonOperator::IsNumeric,
            expected_value: Value::Bool(true),
            depends_on: vec![],
            task_type: EvaluationTaskType::Assertion,
            description: Some("Check that bar is numeric".to_string()),
            result: None,
            condition: false,
        };

        let assert3 = AssertionTask {
            id: "input_baz_check".to_string(),
            field_path: Some("input.baz".to_string()),
            operator: ComparisonOperator::HasLengthEqual,
            expected_value: Value::Number(3.into()),
            depends_on: vec![],
            task_type: EvaluationTaskType::Assertion,
            description: Some("Check that baz has length 3".to_string()),
            result: None,
            condition: false,
        };

        let tasks = EvaluationTasks::new()
            .add_task(assert1)
            .add_task(assert2)
            .add_task(assert3)
            .build();

        let alert_config = GenAIAlertConfig::default();

        let drift_config =
            GenAIDriftConfig::new("scouter", "ML", "0.1.0", 1.0, alert_config, None).unwrap();

        GenAIEvalProfile::new(drift_config, tasks).await.unwrap()
    }

    #[test]
    fn test_evaluator_assert_judge_all_pass() {
        let mut mock = LLMTestServer::new();
        mock.start_server().unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let profile = runtime.block_on(async { create_assert_judge_profile().await });

        assert!(profile.has_llm_tasks());
        assert!(profile.has_assertions());

        let context = serde_json::json!({
            "input": {
                "foo": "bar"
            }
        });

        let record = GenAIEvalRecord::new_rs(
            context,
            Utc::now(),
            "UID123".to_string(),
            "ENTITY123".to_string(),
            None,
        );

        let result_set = runtime.block_on(async {
            GenAIEvaluator::process_event_record(&record, Arc::new(profile)).await
        });

        let eval_set = result_set.unwrap();
        assert!(eval_set.passed_tasks() == 4);
        assert!(eval_set.failed_tasks() == 0);

        mock.stop_server().unwrap();
    }

    #[test]
    fn test_evaluator_assert_one_fail() {
        let mut mock = LLMTestServer::new();
        mock.start_server().unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let profile = runtime.block_on(async { create_assert_profile().await });

        assert!(!profile.has_llm_tasks());
        assert!(profile.has_assertions());

        // we want task "input_bar_check" to fail (is_numeric on non-numeric)
        let context = serde_json::json!({
            "input": {
                "foo": "bar",
                "bar": "not_a_number",
                "baz": [1, 2, 3]
            }
        });

        let record = GenAIEvalRecord::new_rs(
            context,
            Utc::now(),
            "UID123".to_string(),
            "ENTITY123".to_string(),
            None,
        );

        let result_set = runtime.block_on(async {
            GenAIEvaluator::process_event_record(&record, Arc::new(profile)).await
        });

        let eval_set = result_set.unwrap();
        assert!(eval_set.passed_tasks() == 2);
        assert!(eval_set.failed_tasks() == 1);

        mock.stop_server().unwrap();
    }
}
