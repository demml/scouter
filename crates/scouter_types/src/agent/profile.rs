#![allow(clippy::useless_conversion)]
use crate::agent::alert::LLMMetric;
use crate::agent::alert::LLMMetricAlertConfig;
use crate::custom::alert::{CustomMetric, CustomMetricAlertConfig};
use crate::error;
use crate::error::{ProfileError, TypeError};
use crate::util::{json_to_pyobject, pyobject_to_json};
use crate::ProfileRequest;
use crate::{
    DispatchDriftConfig, DriftArgs, DriftType, FileName, ProfileArgs, ProfileBaseArgs,
    ProfileFuncs, DEFAULT_VERSION, MISSING,
};
use core::fmt::Debug;
use potato_head::Agent;
use potato_head::PyWorkflow;
use potato_head::Task;
use potato_head::{Prompt, Provider, Workflow};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::error;

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
    pub alert_config: LLMMetricAlertConfig,

    #[pyo3(get, set)]
    #[serde(default = "default_drift_type")]
    pub drift_type: DriftType,
}

fn default_drift_type() -> DriftType {
    DriftType::Prompt
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
    #[pyo3(signature = (space=MISSING, name=MISSING, version=DEFAULT_VERSION, sample_rate=5, alert_config=LLMMetricAlertConfig::default(), config_path=None))]
    pub fn new(
        space: &str,
        name: &str,
        version: &str,
        sample_rate: usize,
        alert_config: LLMMetricAlertConfig,
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
            drift_type: DriftType::Custom,
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
        ProfileFuncs::__str__(self)
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__json__(self)
    }

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (space=None, name=None, version=None, alert_config=None))]
    pub fn update_config_args(
        &mut self,
        space: Option<String>,
        name: Option<String>,
        version: Option<String>,
        alert_config: Option<LLMMetricAlertConfig>,
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

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct LLMDriftProfile {
    #[pyo3(get)]
    pub config: LLMDriftConfig,

    #[pyo3(get)]
    pub metrics: Vec<LLMMetric>,

    #[pyo3(get)]
    pub scouter_version: String,

    pub workflow: Workflow,
}
#[pymethods]
impl LLMDriftProfile {
    #[new]
    #[pyo3(signature = (config, metrics, scouter_version=None))]
    /// Create a new LLMDriftProfile
    /// LLM evaluations are run asynchronously on the scouter server.
    /// A user will need to create both a workflow that outputs metrics and a list of metrics with
    /// their default values and alert conditions.
    /// # Arguments
    /// * `config` - LLMDriftConfig - The configuration for the LLM drift profile
    /// * `workflow` - Workflow - The workflow that will be used to evaluate the LLM
    /// * `metrics` - Vec<LLMMetric> - The metrics that will be used to evaluate the LLM
    /// * `scouter_version` - Option<String> - The version of scouter that
    /// # Returns
    /// * `Result<Self, ProfileError>` - The LLMDriftProfile
    /// # Errors
    /// * `ProfileError::MissingWorkflowError` - If the workflow is
    pub fn new(
        mut config: LLMDriftConfig,
        metrics: Vec<LLMMetric>,
        scouter_version: Option<String>,
    ) -> Result<Self, ProfileError> {
        let mut workflow = Workflow::new("llm_drift_workflow");
        let mut agents = HashMap::new();

        for metric in &metrics {
            let provider = Provider::from_string(&metric.prompt.model_settings.provider)?;

            let agent = match agents.entry(provider) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => {
                    let agent = Agent::from_model_settings(&metric.prompt.model_settings)?;
                    entry.insert(agent)
                }
            };

            let task = Task::new(&metric.name, metric.prompt.clone(), &agent.id, None, None);
            workflow.add_task(task);
        }

        config.alert_config.set_alert_conditions(&metrics);

        let scouter_version =
            scouter_version.unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());

        Ok(Self {
            config,
            metrics,
            scouter_version,
            workflow,
        })
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__json__(self)
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
        Ok(ProfileFuncs::save_to_json(
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
        alert_config: Option<LLMMetricAlertConfig>,
    ) -> Result<(), TypeError> {
        self.config
            .update_config_args(space, name, version, alert_config)
    }

    /// Create a profile request from the profile
    pub fn create_profile_request(&self) -> Result<ProfileRequest, TypeError> {
        Ok(ProfileRequest {
            space: self.config.space.clone(),
            profile: self.model_dump_json(),
            drift_type: self.config.drift_type.clone(),
        })
    }
}

impl ProfileBaseArgs for LLMDriftProfile {
    fn get_base_args(&self) -> ProfileArgs {
        ProfileArgs {
            name: self.config.name.clone(),
            space: self.config.space.clone(),
            version: self.config.version.clone(),
            schedule: self.config.alert_config.schedule.clone(),
            scouter_version: self.scouter_version.clone(),
            drift_type: self.config.drift_type.clone(),
        }
    }

    fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
}

// write test using mock feature
#[cfg(test)]
#[cfg(feature = "mock")]
mod tests {

    use super::*;
    use crate::AlertThreshold;
    use colored_json::Paint;
    use potato_head::{create_score_prompt, OpenAITestServer};

    #[test]
    fn test_llm_metric_execution() {
        let _runtime = tokio::runtime::Runtime::new().unwrap();
        let mut mock = OpenAITestServer::new();
        mock.start_server().unwrap();
        let prompt = create_score_prompt();

        let metric =
            LLMMetric::new("test_metric", 5.0, prompt, AlertThreshold::Above, None).unwrap();

        //assert!(result.is_ok());

        //let score = result.unwrap();
        //assert_eq!(score.score, 5);

        //mock.stop_server().unwrap();
        // ...test code...
    }
}
