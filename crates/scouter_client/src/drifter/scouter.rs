use crate::data_utils::DataConverterEnum;
use crate::drifter::{
    agent::ClientAgentDrifter, custom::CustomDrifter, psi::PsiDrifter, spc::SpcDrifter,
};
use pyo3::prelude::*;
use pyo3::types::PyList;
use pyo3::IntoPyObjectExt;
use scouter_drift::error::DriftError;
use scouter_drift::spc::SpcDriftMap;
use scouter_types::agent::EvalResultSet;
use scouter_types::spc::SpcDriftProfile;
use scouter_types::{
    agent::{AgentEvalConfig, AgentEvalProfile},
    custom::{CustomDriftProfile, CustomMetric, CustomMetricDriftConfig},
    psi::{PsiDriftConfig, PsiDriftMap, PsiDriftProfile},
    spc::SpcDriftConfig,
    DataType, DriftProfile, DriftType, EvalRecord,
};
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::RwLock;

pub enum DriftMap {
    Spc(SpcDriftMap),
    Psi(PsiDriftMap),
    Agent(EvalResultSet),
}

pub enum DriftConfig {
    Spc(Arc<RwLock<SpcDriftConfig>>),
    Psi(Arc<RwLock<PsiDriftConfig>>),
    Agent(AgentEvalConfig),
    Custom(CustomMetricDriftConfig),
}

impl DriftConfig {
    pub fn spc_config(&self) -> Result<Arc<RwLock<SpcDriftConfig>>, DriftError> {
        match self {
            DriftConfig::Spc(cfg) => Ok(cfg.clone()),
            _ => Err(DriftError::InvalidConfigError),
        }
    }

    pub fn psi_config(&self) -> Result<Arc<RwLock<PsiDriftConfig>>, DriftError> {
        match self {
            DriftConfig::Psi(cfg) => Ok(cfg.clone()),
            _ => Err(DriftError::InvalidConfigError),
        }
    }

    pub fn custom_config(self) -> Result<CustomMetricDriftConfig, DriftError> {
        match self {
            DriftConfig::Custom(cfg) => Ok(cfg),
            _ => Err(DriftError::InvalidConfigError),
        }
    }

    pub fn agent_config(self) -> Result<AgentEvalConfig, DriftError> {
        match self {
            DriftConfig::Agent(cfg) => Ok(cfg),
            _ => Err(DriftError::InvalidConfigError),
        }
    }
}

pub enum Drifter {
    Spc(SpcDrifter),
    Psi(PsiDrifter),
    Custom(CustomDrifter),
    Agent(ClientAgentDrifter),
}

impl Debug for Drifter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Drifter::Spc(_) => write!(f, "SpcDrifter"),
            Drifter::Psi(_) => write!(f, "PsiDrifter"),
            Drifter::Custom(_) => write!(f, "CustomDrifter"),
            Drifter::Agent(_) => write!(f, "AgentDrifter"),
        }
    }
}

impl Drifter {
    fn from_drift_type(drift_type: DriftType) -> Result<Self, DriftError> {
        match drift_type {
            DriftType::Spc => Ok(Drifter::Spc(SpcDrifter::new())),
            DriftType::Psi => Ok(Drifter::Psi(PsiDrifter::new())),
            DriftType::Custom => Ok(Drifter::Custom(CustomDrifter::new())),
            DriftType::Agent => Ok(Drifter::Agent(ClientAgentDrifter::new())),
        }
    }

    fn create_drift_profile<'py>(
        &mut self,
        py: Python<'py>,
        data: &Bound<'py, PyAny>,
        data_type: &DataType,
        config: DriftConfig,
    ) -> Result<DriftProfile, DriftError> {
        match self {
            // Before creating the profile, we first need to do a rough split of the data into string and numeric data types before
            // passing it to the drifter
            Drifter::Spc(drifter) => {
                let data = DataConverterEnum::convert_data(py, data_type, data)?;
                let profile = drifter.create_drift_profile(data, config.spc_config()?)?;
                Ok(DriftProfile::Spc(profile))
            }
            Drifter::Psi(drifter) => {
                let data = DataConverterEnum::convert_data(py, data_type, data)?;
                let profile = drifter.create_drift_profile(data, config.psi_config()?)?;
                Ok(DriftProfile::Psi(profile))
            }
            Drifter::Custom(drifter) => {
                // check if data is pylist. If it is, convert to Vec<CustomMetric>
                // if not extract to CustomMetric and add to vec
                let data = if data.is_instance_of::<PyList>() {
                    data.extract::<Vec<CustomMetric>>()?
                } else {
                    let metric = data.extract::<CustomMetric>()?;
                    vec![metric]
                };

                let profile = drifter.create_drift_profile(config.custom_config()?, data)?;
                Ok(DriftProfile::Custom(profile))
            }
            Drifter::Agent(_drifter) => {
                // Agent eval profiles are created separately, so we will handle this in the create_agent_drift_profile method
                if !data.is_instance_of::<PyList>() {
                    // convert to pylist
                    let type_ = data.get_type().name()?;
                    return Err(DriftError::ExpectedListOfAssertionOrLLMJudgeTasks(
                        type_.to_string(),
                    ));
                };

                let tasks = data.cast::<PyList>()?;
                let profile = AgentEvalProfile::new_py(tasks, Some(config.agent_config()?), None)?;
                Ok(DriftProfile::Agent(profile))
            }
        }
    }

    fn compute_drift<'py>(
        &mut self,
        py: Python<'py>,
        data: &Bound<'py, PyAny>,
        data_type: &DataType,
        profile: &DriftProfile,
    ) -> Result<DriftMap, DriftError> {
        match self {
            Drifter::Spc(drifter) => {
                let data = DataConverterEnum::convert_data(py, data_type, data)?;
                let drift_profile = profile.get_spc_profile()?;
                let drift_map = drifter.compute_drift(data, drift_profile.clone())?;
                Ok(DriftMap::Spc(drift_map))
            }
            Drifter::Psi(drifter) => {
                let data = DataConverterEnum::convert_data(py, data_type, data)?;
                let drift_profile = profile.get_psi_profile()?;
                let drift_map = drifter.compute_drift(data, drift_profile.clone())?;
                Ok(DriftMap::Psi(drift_map))
            }
            Drifter::Custom(_) => {
                // check if data is pylist. If it is, convert to Vec<CustomMetric>
                Err(DriftError::NotImplemented)
            }

            Drifter::Agent(drifter) => {
                // extract data to be Vec<EvalRecord>
                if !data.is_instance_of::<PyList>() {
                    // convert to pylist
                    let type_ = data.get_type().name()?;
                    return Err(DriftError::ExpectedListOfEvalRecords(type_.to_string()));
                };

                let records = data.extract::<Vec<EvalRecord>>()?;
                let records = drifter.compute_drift(records, profile.get_agent_profile()?)?;

                Ok(DriftMap::Agent(EvalResultSet { records }))
            }
        }
    }
}

#[pyclass(name = "Drifter")]
#[derive(Debug, Default)]
pub struct PyDrifter {}

#[pymethods]
impl PyDrifter {
    #[new]
    pub fn new() -> Self {
        Self {}
    }

    /// This method is used to create a drift profile based on the data and config provided
    /// It will automatically infer the data type if not provided
    /// If the config is not provided, it will create a default config based on the drift type
    /// The data can be a numpy array, pandas dataframe, or pyarrow table, Vec<CustomMetric>, or Vec<EvaluationTask>
    /// ## Arguments:
    /// - `data`: The data to create the drift profile from. This can be a numpy array, pandas dataframe, pyarrow table, Vec<CustomMetric>, or Vec<EvaluationTask>.
    /// - `config`: The configuration for the drift profile. This is optional and if not provided, a default configuration will be created based on the drift type.
    /// - `data_type`: The type of the data. This is optional and if not provided, it will be inferred from the data class name.
    #[pyo3(signature = (data, config=None, data_type=None))]
    pub fn create_drift_profile<'py>(
        &self,
        py: Python<'py>,
        data: &Bound<'py, PyAny>,
        config: Option<&Bound<'py, PyAny>>,
        data_type: Option<&DataType>,
    ) -> Result<Bound<'py, PyAny>, DriftError> {
        // if config is None, then we need to create a default config

        let (config_helper, drift_type) = if let Some(obj) = config {
            let drift_type = obj.getattr("drift_type")?.extract::<DriftType>()?;
            let drift_config = match drift_type {
                DriftType::Spc => {
                    let config = obj.extract::<SpcDriftConfig>()?;
                    DriftConfig::Spc(Arc::new(config.into()))
                }
                DriftType::Psi => {
                    let config = obj.extract::<PsiDriftConfig>()?;
                    DriftConfig::Psi(Arc::new(config.into()))
                }
                DriftType::Custom => {
                    let config = obj.extract::<CustomMetricDriftConfig>()?;
                    DriftConfig::Custom(config)
                }
                DriftType::Agent => {
                    let config = obj.extract::<AgentEvalConfig>()?;
                    DriftConfig::Agent(config)
                }
            };
            (drift_config, drift_type)
        } else {
            (
                DriftConfig::Spc(Arc::new(SpcDriftConfig::default().into())),
                DriftType::Spc,
            )
        };

        let mut drift_helper = Drifter::from_drift_type(drift_type)?;

        // if data_type is None, try to infer it from the class name
        // This is for handling, numpy, pandas, pyarrow
        let data_type = match data_type {
            Some(data_type) => data_type,
            None => &DataType::from_object(data).unwrap_or(DataType::Unknown),
        };

        let profile = drift_helper.create_drift_profile(py, data, data_type, config_helper)?;

        match profile {
            DriftProfile::Spc(profile) => Ok(profile.into_bound_py_any(py)?),
            DriftProfile::Psi(profile) => Ok(profile.into_bound_py_any(py)?),
            DriftProfile::Custom(profile) => Ok(profile.into_bound_py_any(py)?),
            DriftProfile::Agent(profile) => Ok(profile.into_bound_py_any(py)?),
        }
    }

    // Specific method for creating GenAI drift profiles
    // This is to avoid confusion with the other drifters
    #[pyo3(signature = (tasks, config=None, alias=None))]
    pub fn create_agent_drift_profile<'py>(
        &mut self,
        py: Python<'py>,
        tasks: &Bound<'py, PyList>,
        config: Option<AgentEvalConfig>,
        alias: Option<String>,
    ) -> Result<Bound<'py, PyAny>, DriftError> {
        let profile = AgentEvalProfile::new_py(tasks, config, alias)?;
        Ok(profile.into_bound_py_any(py)?)
    }

    #[pyo3(signature = (data, drift_profile, data_type=None))]
    pub fn compute_drift<'py>(
        &self,
        py: Python<'py>,
        data: &Bound<'py, PyAny>,
        drift_profile: &Bound<'py, PyAny>,
        data_type: Option<&DataType>,
    ) -> Result<Bound<'py, PyAny>, DriftError> {
        let drift_type = drift_profile
            .getattr("config")?
            .getattr("drift_type")?
            .extract::<DriftType>()?;

        let profile = match drift_type {
            DriftType::Spc => {
                let profile = drift_profile.extract::<SpcDriftProfile>()?;
                DriftProfile::Spc(profile)
            }
            DriftType::Psi => {
                let profile = drift_profile.extract::<PsiDriftProfile>()?;
                DriftProfile::Psi(profile)
            }
            DriftType::Custom => {
                let profile = drift_profile.extract::<CustomDriftProfile>()?;
                DriftProfile::Custom(profile)
            }
            DriftType::Agent => {
                let profile = drift_profile.extract::<AgentEvalProfile>()?;
                DriftProfile::Agent(profile)
            }
        };

        // if data_type is None, try to infer it from the class name
        // This is for handling, numpy, pandas, pyarrow
        // skip if drift_type is LLM, as it will be handled separately

        let data_type = match data_type {
            Some(data_type) => data_type,
            None => {
                if drift_type == DriftType::Agent {
                    // For Agent, we will handle it separately in the create_agent_drift_profile method
                    &DataType::Agent
                } else {
                    &DataType::from_object(data).unwrap_or(DataType::Unknown)
                }
            }
        };

        let mut drift_helper = Drifter::from_drift_type(drift_type)?;

        let drift_map = drift_helper.compute_drift(py, data, data_type, &profile)?;

        match drift_map {
            DriftMap::Spc(map) => Ok(map.into_bound_py_any(py)?),
            DriftMap::Psi(map) => Ok(map.into_bound_py_any(py)?),
            DriftMap::Agent(map) => Ok(map.into_bound_py_any(py)?),
        }
    }
}
