use crate::error::{ProfileError, TypeError};
use crate::llm::alert::LLMAlertConfig;
use crate::llm::alert::LLMDriftMetric;
use crate::util::{json_to_pyobject, pyobject_to_json};
use crate::ProfileRequest;
use crate::{scouter_version, LLMMetricRecord};
use crate::{
    DispatchDriftConfig, DriftArgs, DriftType, FileName, ProfileArgs, ProfileBaseArgs,
    PyHelperFuncs, VersionRequest, DEFAULT_VERSION, MISSING,
};
use core::fmt::Debug;
use potato_head::prompt::ResponseType;
use potato_head::Task;
use potato_head::Workflow;
use potato_head::{Agent, Prompt};
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
use tracing::{debug, error, instrument};

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct LLMDriftConfig {
    #[pyo3(get, set)]
    pub sample_rate: usize,

    #[pyo3(get, set)]
    pub space: String,

    #[pyo3(get, set)]
    pub name: String,

    #[pyo3(get, set)]
    pub version: String,

    #[pyo3(get, set)]
    pub alert_config: LLMAlertConfig,

    #[pyo3(get, set)]
    #[serde(default = "default_drift_type")]
    pub drift_type: DriftType,
}

fn default_drift_type() -> DriftType {
    DriftType::LLM
}

impl DispatchDriftConfig for LLMDriftConfig {
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
impl LLMDriftConfig {
    #[new]
    #[pyo3(signature = (space=MISSING, name=MISSING, version=DEFAULT_VERSION, sample_rate=5, alert_config=LLMAlertConfig::default(), config_path=None))]
    pub fn new(
        space: &str,
        name: &str,
        version: &str,
        sample_rate: usize,
        alert_config: LLMAlertConfig,
        config_path: Option<PathBuf>,
    ) -> Result<Self, ProfileError> {
        if let Some(config_path) = config_path {
            let config = LLMDriftConfig::load_from_json_file(config_path)?;
            return Ok(config);
        }

        Ok(Self {
            sample_rate,
            space: space.to_string(),
            name: name.to_string(),
            version: version.to_string(),
            alert_config,
            drift_type: DriftType::LLM,
        })
    }

    #[staticmethod]
    pub fn load_from_json_file(path: PathBuf) -> Result<LLMDriftConfig, ProfileError> {
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
    #[pyo3(signature = (space=None, name=None, version=None, alert_config=None))]
    pub fn update_config_args(
        &mut self,
        space: Option<String>,
        name: Option<String>,
        version: Option<String>,
        alert_config: Option<LLMAlertConfig>,
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
    metrics: &[LLMDriftMetric],
) -> Result<(), ProfileError> {
    let last_step = execution_order.len() as i32;
    let last_tasks = execution_order
        .get(&last_step)
        .ok_or_else(|| ProfileError::NoTasksFoundError("No final tasks found".to_string()))?;

    for task_id in last_tasks {
        // assert task_id exists in metrics (all output tasks must have a corresponding metric)
        if !metrics.iter().any(|m| m.name == *task_id) {
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
/// - Last tasks have Score response type
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
fn validate_workflow(workflow: &Workflow, metrics: &[LLMDriftMetric]) -> Result<(), ProfileError> {
    let execution_order = workflow.execution_plan()?;

    // Validate first tasks have required parameters
    validate_first_tasks(workflow, &execution_order)?;

    // Validate last tasks have correct response type
    validate_last_tasks(workflow, &execution_order, metrics)?;

    Ok(())
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct LLMDriftProfile {
    #[pyo3(get)]
    pub config: LLMDriftConfig,

    #[pyo3(get)]
    pub metrics: Vec<LLMDriftMetric>,

    #[pyo3(get)]
    pub scouter_version: String,

    pub workflow: Workflow,

    #[pyo3(get)]
    pub metric_names: Vec<String>,
}
#[pymethods]
impl LLMDriftProfile {
    #[new]
    #[pyo3(signature = (config, metrics, workflow=None))]
    /// Create a new LLMDriftProfile
    /// LLM evaluations are run asynchronously on the scouter server.
    ///
    /// # Logic flow:
    ///  1. If a user provides only a list of metrics, a workflow will be created from the metrics using `from_metrics` method.
    ///  2. If a user provides a workflow, It will be parsed and validated using `from_workflow` method.
    ///     - The user must also provide a list of metrics that will be used to evaluate the output of the workflow.
    ///     - The metric names must correspond to the final task names in the workflow
    /// In addition, baseline metrics and threshold will be extracted from the LLMDriftMetric.
    /// # Arguments
    /// * `config` - LLMDriftConfig - The configuration for the LLM drift profile
    /// * `metrics` - Option<Bound<'_, PyList>> - Optional list of metrics that will be used to evaluate the LLM
    /// * `workflow` - Option<Bound<'_, PyAny>> - Optional workflow to use for the LLM drift profile
    ///
    /// # Returns
    /// * `Result<Self, ProfileError>` - The LLMDriftProfile
    ///
    /// # Errors
    /// * `ProfileError::MissingWorkflowError` - If the workflow is
    #[instrument(skip_all)]
    pub fn new(
        config: LLMDriftConfig,
        metrics: Vec<LLMDriftMetric>,
        workflow: Option<Bound<'_, PyAny>>,
    ) -> Result<Self, ProfileError> {
        match workflow {
            Some(py_workflow) => {
                // Extract and validate workflow from Python object
                let workflow = Self::extract_workflow(&py_workflow).map_err(|e| {
                    error!("Failed to extract workflow: {}", e);
                    e
                })?;
                validate_workflow(&workflow, &metrics)?;
                Self::from_workflow(config, workflow, metrics)
            }
            None => {
                // Ensure metrics are provided when no workflow specified
                if metrics.is_empty() {
                    return Err(ProfileError::EmptyMetricsList);
                }
                app_state().block_on(async { Self::from_metrics(config, metrics).await })
            }
        }
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
        let json_str = serde_json::to_string(&self)?;

        let json_value: Value = serde_json::from_str(&json_str)?;

        // Create a new Python dictionary
        let dict = PyDict::new(py);

        // Convert JSON to Python dict
        json_to_pyobject(py, &json_value, &dict)?;

        // Return the Python dictionary
        Ok(dict.into())
    }

    #[pyo3(signature = (path=None))]
    pub fn save_to_json(&self, path: Option<PathBuf>) -> Result<PathBuf, ProfileError> {
        Ok(PyHelperFuncs::save_to_json(
            self,
            path,
            FileName::LLMDriftProfile.to_str(),
        )?)
    }

    #[staticmethod]
    pub fn model_validate(data: &Bound<'_, PyDict>) -> LLMDriftProfile {
        let json_value = pyobject_to_json(data).unwrap();

        let string = serde_json::to_string(&json_value).unwrap();
        serde_json::from_str(&string).expect("Failed to load drift profile")
    }

    #[staticmethod]
    pub fn model_validate_json(json_string: String) -> LLMDriftProfile {
        // deserialize the string to a struct
        serde_json::from_str(&json_string).expect("Failed to load prompt drift profile")
    }

    #[staticmethod]
    pub fn from_file(path: PathBuf) -> Result<LLMDriftProfile, ProfileError> {
        let file = std::fs::read_to_string(&path)?;

        Ok(serde_json::from_str(&file)?)
    }

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (space=None, name=None, version=None, alert_config=None))]
    pub fn update_config_args(
        &mut self,
        space: Option<String>,
        name: Option<String>,
        version: Option<String>,
        alert_config: Option<LLMAlertConfig>,
    ) -> Result<(), TypeError> {
        self.config
            .update_config_args(space, name, version, alert_config)
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
            version_request: VersionRequest {
                version,
                version_type: VersionType::Minor,
                pre_tag: None,
                build_tag: None,
            },
            active: false,
            deactivate_others: false,
        })
    }
}

impl LLMDriftProfile {
    /// Creates an LLMDriftProfile from a configuration and a list of metrics.
    ///
    /// # Arguments
    /// * `config` - LLMDriftConfig - The configuration for the LLM
    /// * `metrics` - Vec<LLMDriftMetric> - The metrics that will be used to evaluate the LLM
    /// * `scouter_version` - Option<String> - The version of scouter that the profile is created with.
    /// # Returns
    /// * `Result<Self, ProfileError>` - The LLMDriftProfile
    pub async fn from_metrics(
        mut config: LLMDriftConfig,
        metrics: Vec<LLMDriftMetric>,
    ) -> Result<Self, ProfileError> {
        // Build a workflow from metrics
        let mut workflow = Workflow::new("llm_drift_workflow");
        let mut agents = HashMap::new();
        let mut metric_names = Vec::new();

        // Create agents. We don't want to duplicate, so we check if the agent already exists.
        // if it doesn't, we create it.
        for metric in &metrics {
            // get prompt (if providing a list of metrics, prompt must be present)
            let prompt = metric
                .prompt
                .as_ref()
                .ok_or_else(|| ProfileError::MissingPromptError(metric.name.clone()))?;

            let provider = prompt.model_settings.provider();

            let agent = match agents.entry(provider) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => {
                    let agent = Agent::from_model_settings(&prompt.model_settings).await?;
                    workflow.add_agent(&agent);
                    entry.insert(agent)
                }
            };

            let task = Task::new(&agent.id, prompt.clone(), &metric.name, None, None);
            validate_prompt_parameters(prompt, &metric.name)?;
            workflow.add_task(task)?;
            metric_names.push(metric.name.clone());
        }

        config.alert_config.set_alert_conditions(&metrics);

        Ok(Self {
            config,
            metrics,
            scouter_version: scouter_version(),
            workflow,
            metric_names,
        })
    }

    /// Creates an LLMDriftProfile from a workflow and a list of metrics.
    /// This is useful when the workflow is already defined and you want to create a profile from it.
    /// This is also for more advanced use cases where the workflow may need to execute many dependent tasks.
    /// Because of this, there are a few requirements:
    /// 1. All beginning tasks in the the workflow must have "input" and/or "output" parameters defined.
    /// 2. All ending tasks in the workflow must have a response type of "Score".
    /// 3. The user must also supply a list of metrics that will be used to evaluate the output of the workflow.
    ///    - The metric names must correspond to the final task names in the workflow.
    ///
    /// # Arguments
    /// * `config` - LLMDriftConfig - The configuration for the LLM
    /// * `workflow` - Workflow - The workflow that will be used to evaluate the L
    /// * `metrics` - Vec<LLMDriftMetric> - The metrics that will be used to evaluate the LLM
    /// * `scouter_version` - Option<String> - The version of scouter that the profile is created with.
    ///
    /// # Returns
    /// * `Result<Self, ProfileError>` - The LLMDriftProfile
    pub fn from_workflow(
        mut config: LLMDriftConfig,
        workflow: Workflow,
        metrics: Vec<LLMDriftMetric>,
    ) -> Result<Self, ProfileError> {
        validate_workflow(&workflow, &metrics)?;

        config.alert_config.set_alert_conditions(&metrics);

        let metric_names = metrics.iter().map(|m| m.name.clone()).collect();

        Ok(Self {
            config,
            metrics,
            scouter_version: scouter_version(),
            workflow,
            metric_names,
        })
    }

    /// Extracts a Workflow from a Python object.
    ///
    /// # Arguments
    /// * `py_workflow` - Python object that should implement `__workflow__()` method
    ///
    /// # Returns
    /// * `Result<Workflow, ProfileError>` - Extracted workflow
    ///
    /// # Errors
    /// * `ProfileError::InvalidWorkflowType` - If object doesn't implement required interface
    #[instrument(skip_all)]
    fn extract_workflow(py_workflow: &Bound<'_, PyAny>) -> Result<Workflow, ProfileError> {
        debug!("Extracting workflow from Python object");

        if !py_workflow.hasattr("__workflow__")? {
            error!("Invalid workflow type provided. Expected object with __workflow__ method.");
            return Err(ProfileError::InvalidWorkflowType);
        }

        let workflow_string = py_workflow
            .getattr("__workflow__")
            .map_err(|e| {
                error!("Failed to call __workflow__ property: {}", e);
                ProfileError::InvalidWorkflowType
            })?
            .extract::<String>()
            .inspect_err(|e| {
                error!(
                    "Failed to extract workflow string from Python object: {}",
                    e
                );
            })?;

        serde_json::from_str(&workflow_string).map_err(|e| {
            error!("Failed to deserialize workflow: {}", e);
            ProfileError::InvalidWorkflowType
        })
    }

    pub fn get_metric_value(&self, metric_name: &str) -> Result<f64, ProfileError> {
        self.metrics
            .iter()
            .find(|m| m.name == metric_name)
            .map(|m| m.value)
            .ok_or_else(|| ProfileError::MetricNotFound(metric_name.to_string()))
    }

    /// Same as py new method, but allows for passing a runtime for async operations.
    /// This is used in Opsml, so that the Opsml global runtime can be used.
    pub fn new_with_runtime(
        config: LLMDriftConfig,
        metrics: Vec<LLMDriftMetric>,
        workflow: Option<Bound<'_, PyAny>>,
        runtime: Arc<tokio::runtime::Runtime>,
    ) -> Result<Self, ProfileError> {
        match workflow {
            Some(py_workflow) => {
                // Extract and validate workflow from Python object
                let workflow = Self::extract_workflow(&py_workflow).map_err(|e| {
                    error!("Failed to extract workflow: {}", e);
                    e
                })?;
                validate_workflow(&workflow, &metrics)?;
                Self::from_workflow(config, workflow, metrics)
            }
            None => {
                // Ensure metrics are provided when no workflow specified
                if metrics.is_empty() {
                    return Err(ProfileError::EmptyMetricsList);
                }
                runtime.block_on(async { Self::from_metrics(config, metrics).await })
            }
        }
    }
}

impl ProfileBaseArgs for LLMDriftProfile {
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
pub struct LLMDriftMap {
    #[pyo3(get)]
    pub records: Vec<LLMMetricRecord>,
}

#[pymethods]
impl LLMDriftMap {
    #[new]
    pub fn new(records: Vec<LLMMetricRecord>) -> Self {
        Self { records }
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
    use crate::AlertThreshold;
    use crate::{AlertDispatchConfig, OpsGenieDispatchConfig, SlackDispatchConfig};
    use potato_head::create_score_prompt;
    use potato_head::prompt::ResponseType;
    use potato_head::Provider;

    use potato_head::{LLMTestServer, Message, PromptContent};

    pub fn create_parameterized_prompt() -> Prompt {
        let user_content =
            PromptContent::Str("What is ${input} + ${response} + ${context}?".to_string());
        let system_content = PromptContent::Str("You are a helpful assistant.".to_string());
        Prompt::new_rs(
            vec![Message::new_rs(user_content)],
            "gpt-4o",
            Provider::OpenAI,
            vec![Message::new_rs(system_content)],
            None,
            None,
            ResponseType::Null,
        )
        .unwrap()
    }

    #[test]
    fn test_llm_drift_config() {
        let mut drift_config = LLMDriftConfig::new(
            MISSING,
            MISSING,
            "0.1.0",
            25,
            LLMAlertConfig::default(),
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
        let new_alert_config = LLMAlertConfig {
            schedule: "0 0 * * * *".to_string(),
            dispatch_config: AlertDispatchConfig::Slack(test_slack_dispatch_config.clone()),
            ..Default::default()
        };

        drift_config
            .update_config_args(None, Some("test".to_string()), None, Some(new_alert_config))
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
    fn test_llm_drift_profile_metric() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let mut mock = LLMTestServer::new();
        mock.start_server().unwrap();
        let prompt = create_score_prompt(Some(vec!["input".to_string()]));

        let metric1 = LLMDriftMetric::new(
            "metric1",
            5.0,
            AlertThreshold::Above,
            None,
            Some(prompt.clone()),
        )
        .unwrap();
        let metric2 = LLMDriftMetric::new(
            "metric2",
            3.0,
            AlertThreshold::Below,
            Some(1.0),
            Some(prompt.clone()),
        )
        .unwrap();

        let llm_metrics = vec![metric1, metric2];

        let alert_config = LLMAlertConfig {
            schedule: "0 0 * * * *".to_string(),
            dispatch_config: AlertDispatchConfig::OpsGenie(OpsGenieDispatchConfig {
                team: "test-team".to_string(),
                priority: "P5".to_string(),
            }),
            ..Default::default()
        };

        let drift_config =
            LLMDriftConfig::new("scouter", "ML", "0.1.0", 25, alert_config, None).unwrap();

        let profile = runtime
            .block_on(async { LLMDriftProfile::from_metrics(drift_config, llm_metrics).await })
            .unwrap();
        let _: Value =
            serde_json::from_str(&profile.model_dump_json()).expect("Failed to parse actual JSON");

        assert_eq!(profile.metrics.len(), 2);
        assert_eq!(profile.scouter_version, env!("CARGO_PKG_VERSION"));

        mock.stop_server().unwrap();
    }

    #[test]
    fn test_llm_drift_profile_workflow() {
        let mut mock = LLMTestServer::new();
        mock.start_server().unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();

        let mut workflow = Workflow::new("My eval Workflow");

        let initial_prompt = create_parameterized_prompt();
        let final_prompt1 = create_score_prompt(None);
        let final_prompt2 = create_score_prompt(None);

        let agent1 = runtime
            .block_on(async { Agent::new(Provider::OpenAI, None).await })
            .unwrap();

        workflow.add_agent(&agent1);

        // First task with parameters
        workflow
            .add_task(Task::new(
                &agent1.id,
                initial_prompt.clone(),
                "task1",
                None,
                None,
            ))
            .unwrap();

        // Final tasks that depend on the first task
        workflow
            .add_task(Task::new(
                &agent1.id,
                final_prompt1.clone(),
                "task2",
                Some(vec!["task1".to_string()]),
                None,
            ))
            .unwrap();

        workflow
            .add_task(Task::new(
                &agent1.id,
                final_prompt2.clone(),
                "task3",
                Some(vec!["task1".to_string()]),
                None,
            ))
            .unwrap();

        let metric1 =
            LLMDriftMetric::new("task2", 3.0, AlertThreshold::Below, Some(1.0), None).unwrap();
        let metric2 =
            LLMDriftMetric::new("task3", 4.0, AlertThreshold::Above, Some(2.0), None).unwrap();

        let llm_metrics = vec![metric1, metric2];

        let alert_config = LLMAlertConfig {
            schedule: "0 0 * * * *".to_string(),
            dispatch_config: AlertDispatchConfig::OpsGenie(OpsGenieDispatchConfig {
                team: "test-team".to_string(),
                priority: "P5".to_string(),
            }),
            ..Default::default()
        };

        let drift_config =
            LLMDriftConfig::new("scouter", "ML", "0.1.0", 25, alert_config, None).unwrap();

        let profile = LLMDriftProfile::from_workflow(drift_config, workflow, llm_metrics).unwrap();

        let _: Value =
            serde_json::from_str(&profile.model_dump_json()).expect("Failed to parse actual JSON");

        assert_eq!(profile.metrics.len(), 2);
        assert_eq!(profile.scouter_version, env!("CARGO_PKG_VERSION"));

        let plan = profile.workflow.execution_plan().unwrap();

        // plan should have 2 steps
        assert_eq!(plan.len(), 2);
        // first step should have 1 task
        assert_eq!(plan.get(&1).unwrap().len(), 1);
        // last step should have 2 tasks
        assert_eq!(plan.get(&(plan.len() as i32)).unwrap().len(), 2);

        mock.stop_server().unwrap();
    }
}
