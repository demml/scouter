use crate::error::{ProfileError, TypeError};
use crate::genai::alert::GenAIAlertConfig;
use crate::genai::eval::{AssertionTask, LLMJudgeTask};
use crate::genai::traits::{ProfileExt, TaskAccessor, TaskRefMut};
use crate::util::{json_to_pyobject, pyobject_to_json, ConfigExt};
use crate::{scouter_version, GenAIEvalTaskResultRecord, GenAIEvalWorkflowRecord};
use crate::{
    DispatchDriftConfig, DriftArgs, DriftType, FileName, ProfileArgs, ProfileBaseArgs,
    PyHelperFuncs, VersionRequest, DEFAULT_VERSION, MISSING,
};
use crate::{GenAIEventRecord, ProfileRequest};
use chrono::{DateTime, Utc};
use core::fmt::Debug;
use potato_head::prompt_types::{Prompt, ResponseType};
use potato_head::Agent;
use potato_head::Workflow;
use potato_head::{create_uuid7, Task};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use scouter_semver::VersionType;
use scouter_state::app_state;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::instrument;

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct GenAIDriftConfig {
    #[pyo3(get, set)]
    pub sample_rate: usize,

    #[pyo3(get, set)]
    pub space: String,

    #[pyo3(get, set)]
    pub name: String,

    #[pyo3(get, set)]
    pub version: String,

    #[pyo3(get, set)]
    pub uid: String,

    #[pyo3(get, set)]
    pub alert_config: GenAIAlertConfig,

    #[pyo3(get, set)]
    #[serde(default = "default_drift_type")]
    pub drift_type: DriftType,
}

impl ConfigExt for GenAIDriftConfig {
    fn space(&self) -> &str {
        &self.space
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        &self.version
    }
}

fn default_drift_type() -> DriftType {
    DriftType::GenAI
}

impl DispatchDriftConfig for GenAIDriftConfig {
    fn get_drift_args(&self) -> DriftArgs {
        DriftArgs {
            name: self.name.clone(),
            space: self.space.clone(),
            version: self.version.clone(),
            dispatch_config: self.alert_config.dispatch_config.clone(),
        }
    }
}

#[pymethods]
#[allow(clippy::too_many_arguments)]
impl GenAIDriftConfig {
    #[new]
    #[pyo3(signature = (space=MISSING, name=MISSING, version=DEFAULT_VERSION, sample_rate=5, alert_config=GenAIAlertConfig::default(), config_path=None))]
    pub fn new(
        space: &str,
        name: &str,
        version: &str,
        sample_rate: usize,
        alert_config: GenAIAlertConfig,
        config_path: Option<PathBuf>,
    ) -> Result<Self, ProfileError> {
        if let Some(config_path) = config_path {
            let config = GenAIDriftConfig::load_from_json_file(config_path)?;
            return Ok(config);
        }

        Ok(Self {
            sample_rate,
            space: space.to_string(),
            name: name.to_string(),
            uid: create_uuid7(),
            version: version.to_string(),
            alert_config,
            drift_type: DriftType::GenAI,
        })
    }

    #[staticmethod]
    pub fn load_from_json_file(path: PathBuf) -> Result<GenAIDriftConfig, ProfileError> {
        // deserialize the string to a struct

        let file = std::fs::read_to_string(&path)?;

        Ok(serde_json::from_str(&file)?)
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__json__(self)
    }

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (space=None, name=None, version=None, uid=None, alert_config=None))]
    pub fn update_config_args(
        &mut self,
        space: Option<String>,
        name: Option<String>,
        version: Option<String>,
        uid: Option<String>,
        alert_config: Option<GenAIAlertConfig>,
    ) -> Result<(), TypeError> {
        if name.is_some() {
            self.name = name.ok_or(TypeError::MissingNameError)?;
        }

        if space.is_some() {
            self.space = space.ok_or(TypeError::MissingSpaceError)?;
        }

        if version.is_some() {
            self.version = version.ok_or(TypeError::MissingVersionError)?;
        }

        if alert_config.is_some() {
            self.alert_config = alert_config.ok_or(TypeError::MissingAlertConfigError)?;
        }

        if uid.is_some() {
            self.uid = uid.ok_or(TypeError::MissingUidError)?;
        }

        Ok(())
    }
}

/// Validates that a prompt contains at least one required parameter.
///
/// LLM evaluation prompts must have either "input" or "output" parameters
/// to access the data being evaluated.
///
/// # Arguments
/// * `prompt` - The prompt to validate
/// * `id` - Identifier for error reporting
///
/// # Returns
/// * `Ok(())` if validation passes
/// * `Err(ProfileError::MissingPromptParametersError)` if no required parameters found
///
/// # Errors
/// Returns an error if the prompt lacks both "input" and "output" parameters.
fn validate_prompt_parameters(prompt: &Prompt, id: &str) -> Result<(), ProfileError> {
    let has_at_least_one_param = !prompt.parameters.is_empty();

    if !has_at_least_one_param {
        return Err(ProfileError::NeedAtLeastOneBoundParameterError(
            id.to_string(),
        ));
    }

    Ok(())
}

/// Validates that a prompt has the correct response type for scoring.
///
/// LLM evaluation prompts must return scores for drift detection.
///
/// # Arguments
/// * `prompt` - The prompt to validate
/// * `id` - Identifier for error reporting
///
/// # Returns
/// * `Ok(())` if validation passes
/// * `Err(ProfileError::InvalidResponseType)` if response type is not Score
fn validate_prompt_response_type(prompt: &Prompt, id: &str) -> Result<(), ProfileError> {
    if prompt.response_type != ResponseType::Score {
        return Err(ProfileError::InvalidResponseType(id.to_string()));
    }

    Ok(())
}

/// Helper function to safely retrieve a task from the workflow.
fn get_workflow_task<'a>(
    workflow: &'a Workflow,
    task_id: &'a str,
) -> Result<&'a Arc<std::sync::RwLock<potato_head::Task>>, ProfileError> {
    workflow
        .task_list
        .tasks
        .get(task_id)
        .ok_or_else(|| ProfileError::NoTasksFoundError(format!("Task '{task_id}' not found")))
}

/// Helper function to validate first tasks in workflow execution.
fn validate_first_tasks(
    workflow: &Workflow,
    execution_order: &HashMap<i32, std::collections::HashSet<String>>,
) -> Result<(), ProfileError> {
    let first_tasks = execution_order
        .get(&1)
        .ok_or_else(|| ProfileError::NoTasksFoundError("No initial tasks found".to_string()))?;

    for task_id in first_tasks {
        let task = get_workflow_task(workflow, task_id)?;
        let task_guard = task
            .read()
            .map_err(|_| ProfileError::NoTasksFoundError("Failed to read task".to_string()))?;

        validate_prompt_parameters(&task_guard.prompt, &task_guard.id)?;
    }

    Ok(())
}

/// Helper function to validate last tasks in workflow execution.
fn validate_last_tasks(
    workflow: &Workflow,
    execution_order: &HashMap<i32, std::collections::HashSet<String>>,
    tasks: &[LLMJudgeTask],
) -> Result<(), ProfileError> {
    let last_step = execution_order.len() as i32;
    let last_tasks = execution_order
        .get(&last_step)
        .ok_or_else(|| ProfileError::NoTasksFoundError("No final tasks found".to_string()))?;

    for task_id in last_tasks {
        // assert task_id exists in tasks (all output tasks must have a corresponding task)
        if !tasks.iter().any(|m| m.id == *task_id) {
            return Err(ProfileError::MetricNotFoundForOutputTask(task_id.clone()));
        }

        let task = get_workflow_task(workflow, task_id)?;
        let task_guard = task
            .read()
            .map_err(|_| ProfileError::NoTasksFoundError("Failed to read task".to_string()))?;

        validate_prompt_response_type(&task_guard.prompt, &task_guard.id)?;
    }

    Ok(())
}

/// Validates workflow execution parameters and response types.
///
/// Ensures that:
/// - First tasks have required prompt parameters
/// - Last tasks have a response type e
///
/// # Arguments
/// * `workflow` - The workflow to validate
///
/// # Returns
/// * `Ok(())` if validation passes
/// * `Err(ProfileError)` if validation fails
///
/// # Errors
/// Returns various ProfileError types based on validation failures.
fn validate_workflow(
    workflow: &Workflow,
    llm_judge_tasks: &[LLMJudgeTask],
) -> Result<(), ProfileError> {
    let execution_order = workflow.execution_plan()?;

    // Validate first tasks have required parameters
    validate_first_tasks(workflow, &execution_order)?;

    // Validate last tasks have correct response type
    validate_last_tasks(workflow, &execution_order, llm_judge_tasks)?;

    Ok(())
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct GenAIEvalProfile {
    #[pyo3(get)]
    pub config: GenAIDriftConfig,

    #[pyo3(get)]
    pub assertions: Vec<AssertionTask>,

    #[pyo3(get)]
    pub llm_judge_tasks: Vec<LLMJudgeTask>,

    #[pyo3(get)]
    pub scouter_version: String,

    pub workflow: Option<Workflow>,
}

#[pymethods]
impl GenAIEvalProfile {
    #[new]
    #[pyo3(signature = (config, assertions=None, llm_judge_tasks=None))]
    /// Create a new GenAIEvalProfile
    /// GenAI evaluations are run asynchronously on the scouter server.
    /// # Arguments
    /// * `config` - GenAIDriftConfig - The configuration for the GenAI drift profile
    /// * `assertions` - Option<Vec<AssertionTask>> - Optional list of assertion tasks
    /// * `llm_judge_tasks` - Option<Vec<LLMJudgeTask>> - Optional list of LLM judge tasks
    /// # Returns
    /// * `Result<Self, ProfileError>` - The GenAIEvalProfile
    /// # Errors
    /// * `ProfileError::MissingWorkflowError` - If the workflow is
    #[instrument(skip_all)]
    pub fn new(
        config: GenAIDriftConfig,
        assertions: Option<Vec<AssertionTask>>,
        llm_judge_tasks: Option<Vec<LLMJudgeTask>>,
    ) -> Result<Self, ProfileError> {
        let assertions = assertions.unwrap_or_default();
        let llm_judge_tasks = llm_judge_tasks.unwrap_or_default();

        if assertions.is_empty() && llm_judge_tasks.is_empty() {
            return Err(ProfileError::EmptyTaskList);
        }

        let workflow = if !llm_judge_tasks.is_empty() {
            let workflow = app_state()
                .block_on(async { Self::build_workflow_from_judges(&llm_judge_tasks).await })?;

            // Validate the workflow
            validate_workflow(&workflow, &llm_judge_tasks)?;
            Some(workflow)
        } else {
            None
        };

        Ok(Self {
            config,
            assertions,
            llm_judge_tasks,
            scouter_version: scouter_version(),
            workflow,
        })
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__json__(self)
    }

    pub fn model_dump(&self, py: Python) -> Result<Py<PyDict>, ProfileError> {
        let json_value = serde_json::to_value(self)?;

        // Create a new Python dictionary
        let dict = PyDict::new(py);

        // Convert JSON to Python dict
        json_to_pyobject(py, &json_value, &dict)?;

        // Return the Python dictionary
        Ok(dict.into())
    }

    #[getter]
    pub fn uid(&self) -> String {
        self.config.uid.clone()
    }

    #[setter]
    pub fn set_uid(&mut self, uid: String) {
        self.config.uid = uid;
    }

    #[pyo3(signature = (path=None))]
    pub fn save_to_json(&self, path: Option<PathBuf>) -> Result<PathBuf, ProfileError> {
        Ok(PyHelperFuncs::save_to_json(
            self,
            path,
            FileName::GenAIEvalProfile.to_str(),
        )?)
    }

    #[staticmethod]
    pub fn model_validate(data: &Bound<'_, PyDict>) -> GenAIEvalProfile {
        let json_value = pyobject_to_json(data).unwrap();

        let string = serde_json::to_string(&json_value).unwrap();
        serde_json::from_str(&string).expect("Failed to load drift profile")
    }

    #[staticmethod]
    pub fn model_validate_json(json_string: String) -> GenAIEvalProfile {
        // deserialize the string to a struct
        serde_json::from_str(&json_string).expect("Failed to load prompt drift profile")
    }

    #[staticmethod]
    pub fn from_file(path: PathBuf) -> Result<GenAIEvalProfile, ProfileError> {
        let file = std::fs::read_to_string(&path)?;

        Ok(serde_json::from_str(&file)?)
    }

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (space=None, name=None, version=None, uid=None, alert_config=None))]
    pub fn update_config_args(
        &mut self,
        space: Option<String>,
        name: Option<String>,
        version: Option<String>,
        uid: Option<String>,
        alert_config: Option<GenAIAlertConfig>,
    ) -> Result<(), TypeError> {
        self.config
            .update_config_args(space, name, version, uid, alert_config)
    }

    /// Create a profile request from the profile
    pub fn create_profile_request(&self) -> Result<ProfileRequest, TypeError> {
        let version: Option<String> = if self.config.version == DEFAULT_VERSION {
            None
        } else {
            Some(self.config.version.clone())
        };

        Ok(ProfileRequest {
            space: self.config.space.clone(),
            profile: self.model_dump_json(),
            drift_type: self.config.drift_type.clone(),
            version_request: Some(VersionRequest {
                version,
                version_type: VersionType::Minor,
                pre_tag: None,
                build_tag: None,
            }),
            active: false,
            deactivate_others: false,
        })
    }

    pub fn has_llm_tasks(&self) -> bool {
        !self.llm_judge_tasks.is_empty()
    }

    /// Check if this profile has assertions
    pub fn has_assertions(&self) -> bool {
        !self.assertions.is_empty()
    }

    /// Get execution order for all tasks (assertions + LLM judges)
    pub fn get_execution_plan(&self) -> Result<Vec<Vec<String>>, ProfileError> {
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();
        let mut in_degree: HashMap<String, usize> = HashMap::new();

        // Add assertions
        for task in &self.assertions {
            graph.insert(task.id.clone(), task.depends_on.clone());
            in_degree.entry(task.id.clone()).or_insert(0);

            for dep in &task.depends_on {
                *in_degree.entry(dep.clone()).or_insert(0) += 1;
            }
        }

        // Add LLM judges
        for task in &self.llm_judge_tasks {
            graph.insert(task.id.clone(), task.depends_on.clone());
            in_degree.entry(task.id.clone()).or_insert(0);

            for dep in &task.depends_on {
                *in_degree.entry(dep.clone()).or_insert(0) += 1;
            }
        }

        // Topological sort for execution order
        let mut result = Vec::new();
        let mut current_level: Vec<String> = in_degree
            .iter()
            .filter(|(_, &degree)| degree == 0)
            .map(|(id, _)| id.clone())
            .collect();

        while !current_level.is_empty() {
            result.push(current_level.clone());

            let mut next_level = Vec::new();
            for task_id in &current_level {
                if let Some(deps) = graph.get(task_id) {
                    for dep in deps {
                        if let Some(degree) = in_degree.get_mut(dep) {
                            *degree -= 1;
                            if *degree == 0 {
                                next_level.push(dep.clone());
                            }
                        }
                    }
                }
            }

            current_level = next_level;
        }

        // Validate that all tasks were included (detect cycles)
        let total_tasks = self.assertions.len() + self.llm_judge_tasks.len();
        let processed_tasks: usize = result.iter().map(|level| level.len()).sum();

        if processed_tasks != total_tasks {
            return Err(ProfileError::CircularDependency);
        }

        Ok(result)
    }
}

impl GenAIEvalProfile {
    /// Build workflow from LLM judge tasks
    /// # Arguments
    /// * `judges` - Slice of LLMJudgeTask
    /// # Returns
    /// * `Result<Workflow, ProfileError>` - The constructed workflow
    async fn build_workflow_from_judges(judges: &[LLMJudgeTask]) -> Result<Workflow, ProfileError> {
        let mut workflow = Workflow::new("genai_evaluation_workflow");
        let mut agents = HashMap::new();

        for judge in judges {
            // Get or create agent for this provider
            let agent = match agents.entry(&judge.prompt.provider) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => {
                    let agent = Agent::new(judge.prompt.provider.clone(), None).await?;
                    workflow.add_agent(&agent);
                    entry.insert(agent)
                }
            };

            let dependencies = if judge.depends_on.is_empty() {
                None
            } else {
                Some(judge.depends_on.clone())
            };

            let task = Task::new(
                &agent.id,
                judge.prompt.clone(),
                &judge.id,
                dependencies,
                None,
            );

            workflow.add_task(task)?;
        }

        Ok(workflow)
    }

    pub fn build_eval_set_from_tasks(
        &self,
        record: &GenAIEventRecord,
        duration_ms: i64,
    ) -> GenAIEvalSet {
        let mut passed_count = 0;
        let mut failed_count = 0;
        let mut records = Vec::new();

        for assertion in &self.assertions {
            if let Some(result) = &assertion.result {
                if result.passed {
                    passed_count += 1;
                } else {
                    failed_count += 1;
                }
                records.push(GenAIEvalTaskResultRecord {
                    created_at: Utc::now(),
                    record_uid: record.uid.clone(),
                    entity_id: record.entity_id.clone(),
                    task_id: assertion.id.clone(),
                    task_type: assertion.task_type.clone(),
                    passed: result.passed,
                    value: result.to_metric_value(),
                    field_path: assertion.field_path.clone(),
                    expected: assertion.expected_value.clone(),
                    actual: result.actual.clone(),
                    message: result.message.clone(),
                    operator: assertion.operator.clone(),
                    // Not applicable for assertions ( we already have entity_id)
                    entity_uid: String::new(),
                });
            }
        }

        for judge in &self.llm_judge_tasks {
            if let Some(result) = &judge.result {
                if result.passed {
                    passed_count += 1;
                } else {
                    failed_count += 1;
                }
                records.push(GenAIEvalTaskResultRecord {
                    created_at: Utc::now(),
                    record_uid: record.uid.clone(),
                    entity_id: record.entity_id.clone(),
                    task_id: judge.id.clone(),
                    task_type: judge.task_type.clone(),
                    passed: result.passed,
                    value: result.to_metric_value(),
                    field_path: judge.field_path.clone(),
                    expected: judge.expected_value.clone(),
                    actual: result.actual.clone(),
                    message: result.message.clone(),
                    operator: judge.operator.clone(),
                    entity_uid: String::new(), // Not applicable for assertions
                });
            }
        }

        let workflow_record = GenAIEvalWorkflowRecord {
            created_at: Utc::now(),
            entity_id: record.entity_id.clone(),
            record_uid: record.uid.clone(),
            total_tasks: (passed_count + failed_count) as i32,
            passed_tasks: passed_count as i32,
            failed_tasks: failed_count as i32,
            pass_rate: if passed_count + failed_count == 0 {
                0.0
            } else {
                (passed_count as f64) / ((passed_count + failed_count) as f64)
            },
            duration_ms: duration_ms as i32,
            entity_uid: String::new(), // Not applicable for assertions
        };

        GenAIEvalSet::new(records, workflow_record)
    }
}

impl ProfileExt for GenAIEvalProfile {
    #[inline]
    fn id(&self) -> &str {
        &self.config.uid
    }

    fn get_task_by_id_mut(&'_ mut self, id: &str) -> Option<TaskRefMut<'_>> {
        if let Some(assertion) = self.assertions.iter_mut().find(|t| t.id() == id) {
            return Some(TaskRefMut::Assertion(assertion));
        }

        if let Some(judge) = self.llm_judge_tasks.iter_mut().find(|t| t.id() == id) {
            return Some(TaskRefMut::LLMJudge(judge));
        }

        None
    }
    #[inline]
    fn get_assertion_by_id(&self, id: &str) -> Option<&AssertionTask> {
        self.assertions.iter().find(|t| t.id() == id)
    }

    #[inline]
    fn get_llm_judge_by_id(&self, id: &str) -> Option<&LLMJudgeTask> {
        self.llm_judge_tasks.iter().find(|t| t.id() == id)
    }

    #[inline]
    fn has_llm_tasks(&self) -> bool {
        !self.llm_judge_tasks.is_empty()
    }
}

impl ProfileBaseArgs for GenAIEvalProfile {
    type Config = GenAIDriftConfig;

    fn config(&self) -> &Self::Config {
        &self.config
    }
    fn get_base_args(&self) -> ProfileArgs {
        ProfileArgs {
            name: self.config.name.clone(),
            space: self.config.space.clone(),
            version: Some(self.config.version.clone()),
            schedule: self.config.alert_config.schedule.clone(),
            scouter_version: self.scouter_version.clone(),
            drift_type: self.config.drift_type.clone(),
        }
    }

    fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GenAIEvalSet {
    #[pyo3(get)]
    pub records: Vec<GenAIEvalTaskResultRecord>,
    pub inner: GenAIEvalWorkflowRecord,
}

impl GenAIEvalSet {
    pub fn new(records: Vec<GenAIEvalTaskResultRecord>, inner: GenAIEvalWorkflowRecord) -> Self {
        Self { records, inner }
    }
}

#[pymethods]
impl GenAIEvalSet {
    #[getter]
    pub fn get_created_at(&self) -> DateTime<Utc> {
        self.inner.created_at
    }

    #[getter]
    pub fn get_record_uid(&self) -> String {
        self.inner.record_uid.clone()
    }

    #[getter]
    pub fn get_total_tasks(&self) -> i32 {
        self.inner.total_tasks
    }

    #[getter]
    pub fn get_passed_tasks(&self) -> i32 {
        self.inner.passed_tasks
    }

    #[getter]
    pub fn get_failed_tasks(&self) -> i32 {
        self.inner.failed_tasks
    }

    #[getter]
    pub fn get_pass_rate(&self) -> f64 {
        self.inner.pass_rate
    }

    #[getter]
    pub fn get_duration_ms(&self) -> i32 {
        self.inner.duration_ms
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }
}

// write test using mock feature
#[cfg(test)]
#[cfg(feature = "mock")]
mod tests {

    use super::*;
    use crate::genai::ComparisonOperator;
    use crate::{AlertDispatchConfig, OpsGenieDispatchConfig, SlackDispatchConfig};
    use potato_head::mock::create_score_prompt;

    #[test]
    fn test_genai_drift_config() {
        let mut drift_config = GenAIDriftConfig::new(
            MISSING,
            MISSING,
            "0.1.0",
            25,
            GenAIAlertConfig::default(),
            None,
        )
        .unwrap();
        assert_eq!(drift_config.name, "__missing__");
        assert_eq!(drift_config.space, "__missing__");
        assert_eq!(drift_config.version, "0.1.0");
        assert_eq!(
            drift_config.alert_config.dispatch_config,
            AlertDispatchConfig::default()
        );

        let test_slack_dispatch_config = SlackDispatchConfig {
            channel: "test-channel".to_string(),
        };
        let new_alert_config = GenAIAlertConfig {
            schedule: "0 0 * * * *".to_string(),
            dispatch_config: AlertDispatchConfig::Slack(test_slack_dispatch_config.clone()),
            ..Default::default()
        };

        drift_config
            .update_config_args(
                None,
                Some("test".to_string()),
                None,
                None,
                Some(new_alert_config),
            )
            .unwrap();

        assert_eq!(drift_config.name, "test");
        assert_eq!(
            drift_config.alert_config.dispatch_config,
            AlertDispatchConfig::Slack(test_slack_dispatch_config)
        );
        assert_eq!(
            drift_config.alert_config.schedule,
            "0 0 * * * *".to_string()
        );
    }

    #[test]
    fn test_genai_drift_profile_metric() {
        let prompt = create_score_prompt(Some(vec!["input".to_string()]));

        let task1 = LLMJudgeTask::new_rs(
            "metric1",
            prompt.clone(),
            Value::Number(4.into()),
            None,
            ComparisonOperator::GreaterThanOrEqual,
            None,
            None,
        );

        let task2 = LLMJudgeTask::new_rs(
            "metric2",
            prompt.clone(),
            Value::Number(2.into()),
            None,
            ComparisonOperator::LessThanOrEqual,
            None,
            None,
        );

        let alert_config = GenAIAlertConfig {
            schedule: "0 0 * * * *".to_string(),
            dispatch_config: AlertDispatchConfig::OpsGenie(OpsGenieDispatchConfig {
                team: "test-team".to_string(),
                priority: "P5".to_string(),
            }),
            ..Default::default()
        };

        let drift_config =
            GenAIDriftConfig::new("scouter", "ML", "0.1.0", 25, alert_config, None).unwrap();

        let profile = GenAIEvalProfile::new(drift_config, None, Some(vec![task1, task2])).unwrap();

        let _: Value =
            serde_json::from_str(&profile.model_dump_json()).expect("Failed to parse actual JSON");

        assert_eq!(profile.llm_judge_tasks.len(), 2);
        assert_eq!(profile.scouter_version, env!("CARGO_PKG_VERSION"));
    }
}
