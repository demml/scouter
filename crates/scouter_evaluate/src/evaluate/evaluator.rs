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
use tracing::{debug, error, instrument};

#[derive(Debug, Clone)]
struct ExecutionContext {
    base_context: Arc<Value>,
    assertion_store: Arc<RwLock<AssertionResultStore>>,
    llm_response_store: Arc<RwLock<LLMResponseStore>>,
    task_registry: Arc<RwLock<TaskRegistry>>,
    task_stages: HashMap<String, i32>,
}

impl ExecutionContext {
    fn new(base_context: Value, registry: TaskRegistry, execution_plan: &Vec<Vec<String>>) -> Self {
        debug!("Creating ExecutionContext");
        Self {
            base_context: Arc::new(base_context),
            assertion_store: Arc::new(RwLock::new(AssertionResultStore::new())),
            llm_response_store: Arc::new(RwLock::new(LLMResponseStore::new())),
            task_registry: Arc::new(RwLock::new(registry)),
            task_stages: Self::build_task_stages(execution_plan),
        }
    }

    fn build_task_stages(execution_plan: &Vec<Vec<String>>) -> HashMap<String, i32> {
        let mut task_stages = HashMap::new();
        for (level_idx, level_tasks) in execution_plan.iter().enumerate() {
            for task_id in level_tasks {
                task_stages.insert(task_id.clone(), level_idx as i32);
            }
        }
        task_stages
    }

    async fn build_scoped_context(&self, depends_on: &[String]) -> Value {
        if depends_on.is_empty() {
            return self.base_context.as_ref().clone();
        }

        let mut scoped_context = self.build_context_map(&self.base_context);
        let registry = self.task_registry.read().await;

        for dep_id in depends_on {
            match registry.get_type(dep_id) {
                Some(TaskType::Assertion) => {
                    let store = self.assertion_store.read().await;
                    if let Some(result) = store.retrieve(dep_id) {
                        scoped_context.insert(dep_id.clone(), result.actual.clone());
                    }
                }
                Some(TaskType::LLMJudge) => {
                    let store = self.llm_response_store.read().await;
                    if let Some(response) = store.retrieve(dep_id) {
                        scoped_context.insert(dep_id.clone(), response.clone());
                    }
                }
                None => {}
            }
        }

        Value::Object(scoped_context)
    }

    fn build_context_map(&self, value: &Value) -> serde_json::Map<String, Value> {
        match value {
            Value::Object(obj) => obj.clone(),
            _ => {
                let mut map = serde_json::Map::new();
                map.insert("context".to_string(), value.clone());
                map
            }
        }
    }

    async fn store_assertion(&self, task_id: String, result: AssertionResult) {
        self.assertion_store.write().await.store(task_id, result);
    }

    async fn store_llm_response(&self, task_id: String, response: Value) {
        self.llm_response_store
            .write()
            .await
            .store(task_id, response);
    }
}

struct DependencyChecker {
    context: ExecutionContext,
}

impl DependencyChecker {
    fn new(context: ExecutionContext) -> Self {
        Self { context }
    }

    async fn check_dependencies_satisfied(&self, task_id: &str) -> Option<bool> {
        debug!("Checking dependencies for task: {}", task_id);
        let dependencies = {
            let registry = self.context.task_registry.read().await;
            match registry.get_dependencies(task_id) {
                Some(deps) => deps,
                None => {
                    // Task exists but has no dependencies - ready to execute
                    debug!("Task '{}' has no dependencies, ready to execute", task_id);
                    return Some(true);
                }
            }
        };

        debug!("Task '{}' has dependencies: {:?}", task_id, dependencies);

        let dep_metadata = {
            let registry = self.context.task_registry.read().await;
            dependencies
                .iter()
                .map(|dep_id| {
                    (
                        dep_id.clone(),
                        registry.is_conditional(dep_id),
                        registry.is_skipped(dep_id),
                    )
                })
                .collect::<Vec<_>>()
        };

        for (dep_id, is_conditional, is_skipped) in dep_metadata {
            debug!(
                "Checking dependency '{}' for task '{}': conditional={}, skipped={}",
                dep_id, task_id, is_conditional, is_skipped
            );
            if is_skipped {
                self.mark_skipped(task_id).await;
                return Some(false);
            }

            let completed = self.check_task_completed(&dep_id).await;
            if !completed {
                if is_conditional {
                    self.mark_skipped(task_id).await;
                    return Some(false);
                }
                return None;
            }

            if is_conditional && !self.check_assertion_passed(&dep_id).await? {
                self.mark_skipped(task_id).await;
                return Some(false);
            }
        }

        Some(true)
    }

    async fn check_task_completed(&self, task_id: &str) -> bool {
        let registry = self.context.task_registry.read().await;
        match registry.get_type(task_id) {
            Some(TaskType::Assertion) => self
                .context
                .assertion_store
                .read()
                .await
                .retrieve(task_id)
                .is_some(),
            Some(TaskType::LLMJudge) => self
                .context
                .llm_response_store
                .read()
                .await
                .retrieve(task_id)
                .is_some(),
            None => false,
        }
    }

    async fn check_assertion_passed(&self, task_id: &str) -> Option<bool> {
        self.context
            .assertion_store
            .read()
            .await
            .retrieve(task_id)
            .map(|res| res.passed)
    }

    async fn mark_skipped(&self, task_id: &str) {
        self.context
            .task_registry
            .write()
            .await
            .mark_skipped(task_id.to_string());
    }

    async fn filter_executable_tasks<'a>(&self, task_ids: &'a [String]) -> Vec<&'a str> {
        debug!("Filtering executable tasks from: {:?}", task_ids);
        let mut executable = Vec::with_capacity(task_ids.len());

        for task_id in task_ids {
            if let Some(true) = self.check_dependencies_satisfied(task_id).await {
                executable.push(task_id.as_str());
            }
        }

        executable
    }
}

struct TaskExecutor {
    context: ExecutionContext,
    profile: Arc<GenAIEvalProfile>,
}

impl TaskExecutor {
    fn new(context: ExecutionContext, profile: Arc<GenAIEvalProfile>) -> Self {
        debug!("Creating TaskExecutor");
        Self { context, profile }
    }

    #[instrument(skip_all)]
    async fn execute_level(&self, task_ids: &[String]) -> Result<(), EvaluationError> {
        let checker = DependencyChecker::new(self.context.clone());
        let executable_tasks = checker.filter_executable_tasks(task_ids).await;

        debug!("Executable tasks for level: {:?}", executable_tasks);

        if executable_tasks.is_empty() {
            return Ok(());
        }

        let (assertions, judges) = self.partition_tasks(executable_tasks).await;

        debug!(
            "Executing level with {} assertions and {} LLM judges",
            assertions.len(),
            judges.len()
        );

        let _result = tokio::try_join!(
            self.execute_assertions(&assertions),
            self.execute_llm_judges(&judges)
        )?;

        Ok(())
    }

    async fn partition_tasks<'a>(&self, task_ids: Vec<&'a str>) -> (Vec<&'a str>, Vec<&'a str>) {
        let registry = self.context.task_registry.read().await;
        let mut assertions = Vec::new();
        let mut judges = Vec::new();

        for id in task_ids {
            match registry.get_type(id) {
                Some(TaskType::Assertion) => assertions.push(id),
                Some(TaskType::LLMJudge) => judges.push(id),
                None => continue,
            }
        }

        (assertions, judges)
    }

    async fn execute_assertions(&self, task_ids: &[&str]) -> Result<(), EvaluationError> {
        debug!("Executing assertion tasks: {:?}", task_ids);
        if task_ids.is_empty() {
            return Ok(());
        }

        let mut join_set = JoinSet::new();

        for &task_id in task_ids {
            let task_id = task_id.to_string();
            let context = self.context.clone();
            let profile = self.profile.clone();

            join_set.spawn(async move {
                Self::execute_assertion_task(&task_id, &context, &profile).await
            });
        }

        while let Some(result) = join_set.join_next().await {
            result.map_err(|e| {
                EvaluationError::GenAIEvaluatorError(format!("Task join error: {}", e))
            })??;
        }

        Ok(())
    }

    async fn execute_llm_judges(&self, task_ids: &[&str]) -> Result<(), EvaluationError> {
        debug!("Executing LLM judge tasks: {:?}", task_ids);
        if task_ids.is_empty() {
            return Ok(());
        }

        let mut join_set = JoinSet::new();

        for &task_id in task_ids {
            let task_id = task_id.to_string();
            let context = self.context.clone();
            let profile = self.profile.clone();

            join_set.spawn(async move {
                let result = Self::execute_llm_judge_task(&task_id, &context, &profile).await;
                result
            });
        }

        let mut results = HashMap::with_capacity(task_ids.len());
        while let Some(result) = join_set.join_next().await {
            let (judge_id, response) = result.map_err(|e| {
                EvaluationError::GenAIEvaluatorError(format!("Task join error: {}", e))
            })??;
            results.insert(judge_id, response);
        }

        self.process_llm_judge_results(results).await?;
        Ok(())
    }

    #[instrument(skip_all, fields(task_id = %task_id))]
    async fn execute_assertion_task(
        task_id: &str,
        context: &ExecutionContext,
        profile: &GenAIEvalProfile,
    ) -> Result<(), EvaluationError> {
        let task = profile
            .get_assertion_by_id(task_id)
            .ok_or_else(|| EvaluationError::TaskNotFound(task_id.to_string()))?;

        let scoped_context = context.build_scoped_context(&task.depends_on).await;
        let result = task.execute(&scoped_context)?;

        context.store_assertion(task_id.to_string(), result).await;
        Ok(())
    }

    #[instrument(skip_all, fields(task_id = %task_id))]
    async fn execute_llm_judge_task(
        task_id: &str,
        context: &ExecutionContext,
        profile: &GenAIEvalProfile,
    ) -> Result<(String, Value), EvaluationError> {
        debug!("Starting LLM judge task: {}", task_id);

        let judge = profile
            .get_llm_judge_by_id(task_id)
            .ok_or_else(|| EvaluationError::TaskNotFound(task_id.to_string()))?;

        debug!("Building scoped context for: {}", task_id);
        let scoped_context = context.build_scoped_context(&judge.depends_on).await;

        let workflow = profile.workflow.as_ref().ok_or_else(|| {
            EvaluationError::GenAIEvaluatorError("No workflow defined".to_string())
        })?;

        debug!("Executing workflow task: {}", task_id);

        // This is where the actual LLM call happens - ensure it's awaited
        let response = workflow
            .execute_task(task_id, &scoped_context)
            .await
            .inspect_err(|e| error!("LLM task {} failed: {:?}", task_id, e))?;

        debug!("Successfully completed LLM judge task: {}", task_id);
        Ok((task_id.to_string(), response))
    }

    async fn process_llm_judge_results(
        &self,
        results: HashMap<String, Value>,
    ) -> Result<(), EvaluationError> {
        for (task_id, response) in results {
            if let Some(task) = self.profile.get_llm_judge_by_id(&task_id) {
                let assertion_result = task.execute(&response)?;

                self.context
                    .store_llm_response(task_id.clone(), response)
                    .await;

                self.context
                    .store_assertion(task_id, assertion_result)
                    .await;
            }
        }
        Ok(())
    }
}

struct ResultCollector {
    context: ExecutionContext,
}

impl ResultCollector {
    fn new(context: ExecutionContext) -> Self {
        Self { context }
    }

    async fn build_eval_set(
        &self,
        record: &GenAIEvalRecord,
        profile: &GenAIEvalProfile,
        duration_ms: i64,
    ) -> GenAIEvalSet {
        let mut passed_count = 0;
        let mut failed_count = 0;
        let mut records = Vec::new();

        let assert_store = self.context.assertion_store.read().await;

        for assertion in &profile.assertion_tasks {
            if let Some(result) = assert_store.retrieve(&assertion.id) {
                if !assertion.condition {
                    if result.passed {
                        passed_count += 1;
                    } else {
                        failed_count += 1;
                    }
                }

                let stage = *self.context.task_stages.get(&assertion.id).unwrap_or(&-1);

                records.push(scouter_types::GenAIEvalTaskResult {
                    created_at: chrono::Utc::now(),
                    record_uid: record.uid.clone(),
                    entity_id: record.entity_id,
                    task_id: assertion.id.clone(),
                    task_type: assertion.task_type.clone(),
                    passed: result.passed,
                    value: result.to_metric_value(),
                    field_path: assertion.field_path.clone(),
                    expected: result.expected.clone(),
                    actual: result.actual.clone(),
                    message: result.message.clone(),
                    operator: assertion.operator.clone(),
                    entity_uid: String::new(),
                    condition: assertion.condition,
                    stage,
                });
            }
        }

        for judge in &profile.llm_judge_tasks {
            if let Some(result) = assert_store.retrieve(&judge.id) {
                if !judge.condition {
                    if result.passed {
                        passed_count += 1;
                    } else {
                        failed_count += 1;
                    }
                }

                let stage = *self.context.task_stages.get(&judge.id).unwrap_or(&-1);

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
                    condition: judge.condition,
                    stage,
                });
            }
        }

        let workflow_record = scouter_types::GenAIEvalWorkflowResult {
            created_at: chrono::Utc::now(),
            entity_id: record.entity_id,
            record_uid: record.uid.clone(),
            total_tasks: passed_count + failed_count,
            passed_tasks: passed_count,
            failed_tasks: failed_count,
            pass_rate: if passed_count + failed_count == 0 {
                0.0
            } else {
                passed_count as f64 / (passed_count + failed_count) as f64
            },
            duration_ms,
            entity_uid: String::new(),
        };

        GenAIEvalSet::new(records, workflow_record)
    }
}

pub struct GenAIEvaluator;

impl GenAIEvaluator {
    #[instrument(skip_all, fields(record_uid = %record.uid))]
    pub async fn process_event_record(
        record: &GenAIEvalRecord,
        profile: Arc<GenAIEvalProfile>,
    ) -> Result<GenAIEvalSet, EvaluationError> {
        let begin = chrono::Utc::now();

        let mut registry = TaskRegistry::new();
        Self::register_tasks(&mut registry, &profile);

        let execution_plan = profile.get_execution_plan()?;
        let context = ExecutionContext::new(record.context.clone(), registry, &execution_plan);
        let executor = TaskExecutor::new(context.clone(), profile.clone());

        debug!(
            "Starting evaluation for record: {} with {} levels",
            record.uid,
            execution_plan.len()
        );

        for (level_idx, level_tasks) in execution_plan.iter().enumerate() {
            debug!(
                "Executing level {} with {} tasks",
                level_idx,
                level_tasks.len()
            );
            executor
                .execute_level(level_tasks)
                .await
                .inspect_err(|e| error!("Failed to execute level {}: {:?}", level_idx, e))?;
        }

        let end = chrono::Utc::now();
        let duration_ms = (end - begin).num_milliseconds();

        let collector = ResultCollector::new(context);
        let eval_set = collector
            .build_eval_set(record, &profile, duration_ms)
            .await;

        Ok(eval_set)
    }

    fn register_tasks(registry: &mut TaskRegistry, profile: &GenAIEvalProfile) {
        for task in &profile.assertion_tasks {
            registry.register(task.id.clone(), TaskType::Assertion, task.condition);
            if !task.depends_on.is_empty() {
                registry.register_dependencies(task.id.clone(), task.depends_on.clone());
            }
        }

        for task in &profile.llm_judge_tasks {
            registry.register(task.id.clone(), TaskType::LLMJudge, task.condition);
            if !task.depends_on.is_empty() {
                registry.register_dependencies(task.id.clone(), task.depends_on.clone());
            }
        }
    }
}
