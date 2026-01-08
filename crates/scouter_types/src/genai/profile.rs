use crate::error::{ProfileError, TypeError};
use crate::genai::alert::GenAIAlertConfig;
use crate::genai::eval::{AssertionTask, EvaluationTask, LLMJudgeTask};
use crate::genai::traits::{separate_tasks, ProfileExt, TaskAccessor};
use crate::genai::utils::extract_assertion_tasks_from_pylist;
use crate::util::{json_to_pyobject, pyobject_to_json, ConfigExt};
use crate::{
    scouter_version, GenAIEvalTaskResult, GenAIEvalWorkflowResult, WorkflowResultTableEntry,
};
use crate::{
    DispatchDriftConfig, DriftArgs, DriftType, FileName, ProfileArgs, ProfileBaseArgs,
    PyHelperFuncs, VersionRequest, DEFAULT_VERSION, MISSING,
};
use crate::{ProfileRequest, TaskResultTableEntry};
use chrono::{DateTime, Utc};
use core::fmt::Debug;
use owo_colors::OwoColorize;
use potato_head::prompt_types::Prompt;
use potato_head::Agent;
use potato_head::Workflow;
use potato_head::{create_uuid7, Task};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use scouter_semver::VersionType;
use scouter_state::app_state;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::hash_map::Entry;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tabled::{
    settings::{object::Rows, Alignment, Color, Format, Style},
    Table,
};
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

impl Default for GenAIDriftConfig {
    fn default() -> Self {
        Self {
            sample_rate: 5,
            space: "default".to_string(),
            name: "default_genai_profile".to_string(),
            version: DEFAULT_VERSION.to_string(),
            uid: create_uuid7(),
            alert_config: GenAIAlertConfig::default(),
            drift_type: DriftType::GenAI,
        }
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
fn validate_workflow(workflow: &Workflow) -> Result<(), ProfileError> {
    let execution_order = workflow.execution_plan()?;

    // Validate first tasks have required parameters
    validate_first_tasks(workflow, &execution_order)?;

    Ok(())
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct GenAIEvalProfile {
    #[pyo3(get)]
    pub config: GenAIDriftConfig,

    #[pyo3(get)]
    pub assertion_tasks: Vec<AssertionTask>,

    #[pyo3(get)]
    pub llm_judge_tasks: Vec<LLMJudgeTask>,

    #[pyo3(get)]
    pub scouter_version: String,

    pub workflow: Option<Workflow>,

    pub task_ids: BTreeSet<String>,
}

#[pymethods]
impl GenAIEvalProfile {
    #[new]
    #[pyo3(signature = (config, tasks))]
    /// Create a new GenAIEvalProfile
    /// GenAI evaluations are run asynchronously on the scouter server.
    /// # Arguments
    /// * `config` - GenAIDriftConfig - The configuration for the GenAI drift profile
    /// * `tasks` - PyList - List of AssertionTask, LLMJudgeTask or ConditionalTask
    /// # Returns
    /// * `Result<Self, ProfileError>` - The GenAIEvalProfile
    /// # Errors
    /// * `ProfileError::MissingWorkflowError` - If the workflow is
    #[instrument(skip_all)]
    pub fn new_py(
        config: GenAIDriftConfig,
        tasks: &Bound<'_, PyList>,
    ) -> Result<Self, ProfileError> {
        let (assertion_tasks, llm_judge_tasks) = extract_assertion_tasks_from_pylist(tasks)?;

        let (workflow, task_ids) = app_state()
            .block_on(async { Self::build_profile(&llm_judge_tasks, &assertion_tasks).await })?;

        Ok(Self {
            config,
            assertion_tasks,
            llm_judge_tasks,

            scouter_version: scouter_version(),
            workflow,
            task_ids,
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
        !self.assertion_tasks.is_empty()
    }

    /// Get execution order for all tasks (assertions + LLM judges)
    pub fn get_execution_plan(&self) -> Result<Vec<Vec<String>>, ProfileError> {
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();
        let mut in_degree: HashMap<String, usize> = HashMap::new();

        // Initialize all tasks with empty dependency lists and 0 in-degree
        for task in &self.assertion_tasks {
            graph.entry(task.id.clone()).or_default();
            in_degree.entry(task.id.clone()).or_insert(0);
        }
        for task in &self.llm_judge_tasks {
            graph.entry(task.id.clone()).or_default();
            in_degree.entry(task.id.clone()).or_insert(0);
        }

        // Build dependency graph correctly:
        // For each dependency, add current task as a dependent
        for task in &self.assertion_tasks {
            for dep in &task.depends_on {
                // dep -> task_id (dep must execute before task_id)
                graph.entry(dep.clone()).or_default().push(task.id.clone());
                *in_degree.entry(task.id.clone()).or_insert(0) += 1;
            }
        }

        for task in &self.llm_judge_tasks {
            for dep in &task.depends_on {
                // dep -> task_id (dep must execute before task_id)
                graph.entry(dep.clone()).or_default().push(task.id.clone());
                *in_degree.entry(task.id.clone()).or_insert(0) += 1;
            }
        }

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
                // For each task we just completed, reduce in-degree of its dependents
                if let Some(dependents) = graph.get(task_id) {
                    for dependent in dependents {
                        if let Some(degree) = in_degree.get_mut(dependent) {
                            *degree -= 1;
                            if *degree == 0 {
                                next_level.push(dependent.clone());
                            }
                        }
                    }
                }
            }

            current_level = next_level;
        }

        let total_tasks = self.assertion_tasks.len() + self.llm_judge_tasks.len();
        let processed_tasks: usize = result.iter().map(|level| level.len()).sum();

        if processed_tasks != total_tasks {
            return Err(ProfileError::CircularDependency);
        }

        Ok(result)
    }

    pub fn print_execution_plan(&self) -> Result<(), ProfileError> {
        use owo_colors::OwoColorize;

        let plan = self.get_execution_plan()?;

        println!("\n{}", "Evaluation Execution Plan".bold().green());
        println!("{}", "═".repeat(70).green());

        let mut conditional_count = 0;

        for (level_idx, level) in plan.iter().enumerate() {
            let stage_label = format!("Stage {}", level_idx + 1);
            println!("\n{}", stage_label.bold().cyan());

            for (task_idx, task_id) in level.iter().enumerate() {
                let is_last = task_idx == level.len() - 1;
                let prefix = if is_last { "└─" } else { "├─" };

                let task = self.get_task_by_id(task_id).ok_or_else(|| {
                    ProfileError::NoTasksFoundError(format!("Task '{}' not found", task_id))
                })?;

                let is_conditional = if let Some(assertion) = self.get_assertion_by_id(task_id) {
                    assertion.condition
                } else if let Some(judge) = self.get_llm_judge_by_id(task_id) {
                    judge.condition
                } else {
                    false
                };

                if is_conditional {
                    conditional_count += 1;
                }

                let (task_type, color_fn): (&str, fn(&str) -> String) =
                    if self.assertion_tasks.iter().any(|t| &t.id == task_id) {
                        ("Assertion", |s: &str| s.yellow().to_string())
                    } else {
                        ("LLM Judge", |s: &str| s.purple().to_string())
                    };

                let conditional_marker = if is_conditional {
                    " [CONDITIONAL]".bright_red().to_string()
                } else {
                    String::new()
                };

                println!(
                    "{} {} ({}){}",
                    prefix,
                    task_id.bold(),
                    color_fn(task_type),
                    conditional_marker
                );

                let deps = task.depends_on();
                if !deps.is_empty() {
                    let dep_prefix = if is_last { "  " } else { "│ " };

                    let (conditional_deps, normal_deps): (Vec<_>, Vec<_>) =
                        deps.iter().partition(|dep_id| {
                            self.get_assertion_by_id(dep_id)
                                .map(|t| t.condition)
                                .or_else(|| self.get_llm_judge_by_id(dep_id).map(|t| t.condition))
                                .unwrap_or(false)
                        });

                    if !normal_deps.is_empty() {
                        println!(
                            "{}   {} {}",
                            dep_prefix,
                            "depends on:".dimmed(),
                            normal_deps
                                .iter()
                                .map(|s| s.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                                .dimmed()
                        );
                    }

                    if !conditional_deps.is_empty() {
                        println!(
                            "{}   {} {}",
                            dep_prefix,
                            "▶ conditional gate:".bright_red().dimmed(),
                            conditional_deps
                                .iter()
                                .map(|d| format!("{} must pass", d))
                                .collect::<Vec<_>>()
                                .join(", ")
                                .red()
                                .dimmed()
                        );
                    }
                }

                if is_conditional {
                    let continuation = if is_last { "  " } else { "│ " };
                    println!(
                        "{}   {} {}",
                        continuation,
                        "▶".bright_red(),
                        "creates conditional branch".bright_red().dimmed()
                    );
                }
            }
        }

        println!("\n{}", "═".repeat(70).green());
        println!(
            "{}: {} tasks across {} stages",
            "Summary".bold(),
            self.assertion_tasks.len() + self.llm_judge_tasks.len(),
            plan.len()
        );

        if conditional_count > 0 {
            println!(
                "{}: {} conditional tasks that create execution branches",
                "Branches".bold().bright_red(),
                conditional_count
            );
        }

        println!();

        Ok(())
    }
}

impl GenAIEvalProfile {
    #[instrument(skip_all)]
    pub async fn new(
        config: GenAIDriftConfig,
        tasks: Vec<EvaluationTask>,
    ) -> Result<Self, ProfileError> {
        let (assertion_tasks, llm_judge_tasks) = separate_tasks(tasks);
        let (workflow, task_ids) = Self::build_profile(&llm_judge_tasks, &assertion_tasks).await?;

        Ok(Self {
            config,
            assertion_tasks,
            llm_judge_tasks,

            scouter_version: scouter_version(),
            workflow,
            task_ids,
        })
    }

    async fn build_profile(
        judge_tasks: &[LLMJudgeTask],
        assertion_tasks: &[AssertionTask],
    ) -> Result<(Option<Workflow>, BTreeSet<String>), ProfileError> {
        if assertion_tasks.is_empty() && judge_tasks.is_empty() {
            return Err(ProfileError::EmptyTaskList);
        }

        let workflow = if !judge_tasks.is_empty() {
            let assertion_ids: BTreeSet<String> =
                assertion_tasks.iter().map(|t| t.id.clone()).collect();
            let workflow = Self::build_workflow_from_judges(judge_tasks, &assertion_ids).await?;

            // Validate the workflow
            validate_workflow(&workflow)?;
            Some(workflow)
        } else {
            None
        };

        // Validate LLM judge prompts individually
        for judge in judge_tasks {
            validate_prompt_parameters(&judge.prompt, &judge.id)?;
        }

        // Collect all task IDs from assertion and LLM judge tasks
        let mut task_ids = BTreeSet::new();
        for task in assertion_tasks {
            task_ids.insert(task.id.clone());
        }
        for task in judge_tasks {
            task_ids.insert(task.id.clone());
        }

        // check for duplicate task IDs across all task types
        let total_tasks = assertion_tasks.len() + judge_tasks.len();

        if task_ids.len() != total_tasks {
            return Err(ProfileError::DuplicateTaskIds);
        }
        Ok((workflow, task_ids))
    }

    /// Build workflow from LLM judge tasks
    /// # Arguments
    /// * `judges` - Slice of LLMJudgeTask
    /// # Returns
    /// * `Result<Workflow, ProfileError>` - The constructed workflow
    pub async fn build_workflow_from_judges(
        judges: &[LLMJudgeTask],
        assertion_ids: &BTreeSet<String>,
    ) -> Result<Workflow, ProfileError> {
        let mut workflow = Workflow::new(&format!("eval_workflow_{}", create_uuid7()));
        let mut agents = HashMap::new();

        for judge in judges {
            let agent = match agents.entry(&judge.prompt.provider) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => {
                    let agent = Agent::new(judge.prompt.provider.clone(), None).await?;
                    workflow.add_agent(&agent);
                    entry.insert(agent)
                }
            };

            // filter out assertion IDs from dependencies
            // All dependencies are checked when building a workflow, and non-judge tasks are not part of the workflow
            let workflow_dependencies: Vec<String> = judge
                .depends_on
                .iter()
                .filter(|dep_id| !assertion_ids.contains(*dep_id))
                .cloned()
                .collect();

            let task_deps = if workflow_dependencies.is_empty() {
                None
            } else {
                Some(workflow_dependencies)
            };

            let task = Task::new(
                &agent.id,
                judge.prompt.clone(),
                &judge.id,
                task_deps,
                judge.max_retries,
            );

            workflow.add_task(task)?;
        }

        Ok(workflow)
    }
}

impl ProfileExt for GenAIEvalProfile {
    #[inline]
    fn id(&self) -> &str {
        &self.config.uid
    }

    fn get_task_by_id(&self, id: &str) -> Option<&dyn TaskAccessor> {
        if let Some(assertion) = self.assertion_tasks.iter().find(|t| t.id() == id) {
            return Some(assertion);
        }

        if let Some(judge) = self.llm_judge_tasks.iter().find(|t| t.id() == id) {
            return Some(judge);
        }

        None
    }

    #[inline]
    /// Get assertion task by ID, first checking AssertionTasks, then ConditionalTasks
    fn get_assertion_by_id(&self, id: &str) -> Option<&AssertionTask> {
        self.assertion_tasks.iter().find(|t| t.id() == id)
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
    pub records: Vec<GenAIEvalTaskResult>,
    pub inner: GenAIEvalWorkflowResult,
}

impl GenAIEvalSet {
    pub fn build_task_entries(&self) -> Vec<TaskResultTableEntry> {
        self.records
            .iter()
            .map(|record| record.to_table_entry())
            .collect()
    }
    pub fn build_workflow_entries(&self) -> Vec<WorkflowResultTableEntry> {
        vec![self.inner.to_table_entry()]
    }
    fn build_tasks_table(&self) -> Table {
        let entries: Vec<TaskResultTableEntry> = self.build_task_entries();

        let mut table = Table::new(entries);
        table.with(Style::sharp());

        table.modify(
            Rows::new(0..1),
            (
                Format::content(|s: &str| s.truecolor(245, 77, 85).bold().to_string()),
                Alignment::center(),
                Color::BOLD,
            ),
        );
        table
    }

    fn build_workflow_table(&self) -> Table {
        let entries: Vec<WorkflowResultTableEntry> = self.build_workflow_entries();

        let mut table = Table::new(entries);
        table.with(Style::sharp());

        table.modify(
            Rows::new(0..1),
            (
                Format::content(|s: &str| s.truecolor(245, 77, 85).bold().to_string()),
                Alignment::center(),
                Color::BOLD,
            ),
        );
        table
    }

    pub fn new(records: Vec<GenAIEvalTaskResult>, inner: GenAIEvalWorkflowResult) -> Self {
        Self { records, inner }
    }

    pub fn empty() -> Self {
        Self {
            records: Vec::new(),
            inner: GenAIEvalWorkflowResult {
                created_at: Utc::now(),
                record_uid: String::new(),
                entity_id: 0,
                total_tasks: 0,
                passed_tasks: 0,
                failed_tasks: 0,
                pass_rate: 0.0,
                duration_ms: 0,
                entity_uid: String::new(),
            },
        }
    }
}

#[pymethods]
impl GenAIEvalSet {
    #[getter]
    pub fn created_at(&self) -> DateTime<Utc> {
        self.inner.created_at
    }

    #[getter]
    pub fn record_uid(&self) -> String {
        self.inner.record_uid.clone()
    }

    #[getter]
    pub fn total_tasks(&self) -> i32 {
        self.inner.total_tasks
    }

    #[getter]
    pub fn passed_tasks(&self) -> i32 {
        self.inner.passed_tasks
    }

    #[getter]
    pub fn failed_tasks(&self) -> i32 {
        self.inner.failed_tasks
    }

    #[getter]
    pub fn pass_rate(&self) -> f64 {
        self.inner.pass_rate
    }

    #[getter]
    pub fn duration_ms(&self) -> i64 {
        self.inner.duration_ms
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }

    #[pyo3(signature = (show_tasks=false))]
    /// Display results as a table in the console
    /// # Arguments
    /// * `show_tasks` - If true, display detailed task results; otherwise, show workflow summary
    pub fn as_table(&self, show_tasks: bool) {
        if show_tasks {
            let tasks_table = self.build_tasks_table();
            println!("\n{}", "Task Details".truecolor(245, 77, 85).bold());
            println!("{}", tasks_table);
        } else {
            let workflow_table = self.build_workflow_table();
            println!("\n{}", "Workflow Summary".truecolor(245, 77, 85).bold());
            println!("{}", workflow_table);
        }
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GenAIEvalResultSet {
    #[pyo3(get)]
    pub records: Vec<GenAIEvalSet>,
}

#[pymethods]
impl GenAIEvalResultSet {
    pub fn record(&self, id: &str) -> Option<GenAIEvalSet> {
        self.records.iter().find(|r| r.record_uid() == id).cloned()
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
    use crate::genai::{ComparisonOperator, EvaluationTasks};
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

    #[tokio::test]
    async fn test_genai_drift_profile_metric() {
        let prompt = create_score_prompt(Some(vec!["input".to_string()]));

        let task1 = LLMJudgeTask::new_rs(
            "metric1",
            prompt.clone(),
            Value::Number(4.into()),
            None,
            ComparisonOperator::GreaterThanOrEqual,
            None,
            None,
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
            None,
            None,
        );

        let tasks = EvaluationTasks::new()
            .add_task(task1)
            .add_task(task2)
            .build();

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

        let profile = GenAIEvalProfile::new(drift_config, tasks).await.unwrap();

        let _: Value =
            serde_json::from_str(&profile.model_dump_json()).expect("Failed to parse actual JSON");

        assert_eq!(profile.llm_judge_tasks.len(), 2);
        assert_eq!(profile.scouter_version, env!("CARGO_PKG_VERSION"));
    }
}
