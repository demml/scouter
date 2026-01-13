use crate::error::EvaluationError;
use crate::evaluate::store::{AssertionResultStore, LLMResponseStore, TaskRegistry, TaskType};
use crate::tasks::traits::EvaluationTask;
use chrono::{DateTime, Utc};
use scouter_types::genai::traits::ProfileExt;
use scouter_types::genai::{AssertionResult, ExecutionPlan, GenAIEvalProfile, GenAIEvalSet};
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
    fn new(base_context: Value, registry: TaskRegistry, execution_plan: &ExecutionPlan) -> Self {
        debug!("Creating ExecutionContext");
        Self {
            base_context: Arc::new(base_context),
            assertion_store: Arc::new(RwLock::new(AssertionResultStore::new())),
            llm_response_store: Arc::new(RwLock::new(LLMResponseStore::new())),
            task_registry: Arc::new(RwLock::new(registry)),
            task_stages: Self::build_task_stages(execution_plan),
        }
    }

    fn build_task_stages(execution_plan: &ExecutionPlan) -> HashMap<String, i32> {
        execution_plan
            .nodes
            .iter()
            .map(|(id, node)| (id.clone(), node.stage as i32))
            .collect()
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
                        scoped_context.insert(dep_id.clone(), result.2.actual.clone());
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

    async fn store_assertion(
        &self,
        task_id: String,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        result: AssertionResult,
    ) {
        self.assertion_store
            .write()
            .await
            .store(task_id, start_time, end_time, result);
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
            .map(|res| res.2.passed)
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
            let (judge_id, start_time, response) = result.map_err(|e| {
                EvaluationError::GenAIEvaluatorError(format!("Task join error: {}", e))
            })??;
            results.insert(judge_id, (start_time, response));
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
        let start_time = Utc::now();

        let task = profile
            .get_assertion_by_id(task_id)
            .ok_or_else(|| EvaluationError::TaskNotFound(task_id.to_string()))?;

        let scoped_context = context.build_scoped_context(&task.depends_on).await;
        let result = task.execute(&scoped_context)?;

        let end_time = Utc::now();
        context
            .store_assertion(task_id.to_string(), start_time, end_time, result)
            .await;
        Ok(())
    }

    #[instrument(skip_all, fields(task_id = %task_id))]
    async fn execute_llm_judge_task(
        task_id: &str,
        context: &ExecutionContext,
        profile: &GenAIEvalProfile,
    ) -> Result<(String, DateTime<Utc>, serde_json::Value), EvaluationError> {
        debug!("Starting LLM judge task: {}", task_id);
        let start_time = Utc::now();
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
        Ok((task_id.to_string(), start_time, response))
    }

    async fn process_llm_judge_results(
        &self,
        results: HashMap<String, (DateTime<Utc>, Value)>,
    ) -> Result<(), EvaluationError> {
        for (task_id, (start_time, response)) in results {
            if let Some(task) = self.profile.get_llm_judge_by_id(&task_id) {
                let assertion_result = task.execute(&response)?;

                self.context
                    .store_llm_response(task_id.clone(), response)
                    .await;

                self.context
                    .store_assertion(task_id, start_time, Utc::now(), assertion_result)
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
        execution_plan: ExecutionPlan,
    ) -> GenAIEvalSet {
        let mut passed_count = 0;
        let mut failed_count = 0;
        let mut records = Vec::new();

        let assert_store = self.context.assertion_store.read().await;

        for assertion in &profile.assertion_tasks {
            if let Some((start_time, end_time, result)) = assert_store.retrieve(&assertion.id) {
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
                    start_time,
                    end_time,
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
            if let Some((start_time, end_time, result)) = assert_store.retrieve(&judge.id) {
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
                    start_time,
                    end_time,
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
            id: record.id,
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
            execution_plan,
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
            "Starting evaluation for record: {} with {} stages",
            record.uid,
            execution_plan.stages.len()
        );

        for (stage_idx, stage_tasks) in execution_plan.stages.iter().enumerate() {
            debug!(
                "Executing stage {} with {} tasks",
                stage_idx,
                stage_tasks.len()
            );
            executor
                .execute_level(stage_tasks)
                .await
                .inspect_err(|e| error!("Failed to execute stage {}: {:?}", stage_idx, e))?;
        }

        let end = chrono::Utc::now();
        let duration_ms = (end - begin).num_milliseconds();

        let collector = ResultCollector::new(context);
        let eval_set = collector
            .build_eval_set(record, &profile, duration_ms, execution_plan)
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
            "foo": "bar" }
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
                "baz": [1, 2, 3]}
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
