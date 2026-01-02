use crate::data_utils::DataConverterEnum;
use crate::drifter::{
    custom::CustomDrifter, genai::ClientGenAIDrifter, psi::PsiDrifter, spc::SpcDrifter,
};
use pyo3::prelude::*;
use pyo3::types::PyList;
use pyo3::IntoPyObjectExt;
use scouter_drift::error::DriftError;
use scouter_drift::spc::SpcDriftMap;
use scouter_types::genai::{AssertionTask, GenAIDriftMap, GenAIDriftMetric, LLMJudgeTask};
use scouter_types::spc::SpcDriftProfile;
use scouter_types::GenAIRecord;
use scouter_types::{
    custom::{CustomDriftProfile, CustomMetric, CustomMetricDriftConfig},
    genai::{GenAIDriftConfig, GenAIEvalProfile},
    psi::{PsiDriftConfig, PsiDriftMap, PsiDriftProfile},
    spc::SpcDriftConfig,
    DataType, DriftProfile, DriftType,
};
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::RwLock;

pub enum DriftMap {
    Spc(SpcDriftMap),
    Psi(PsiDriftMap),
    GenAI(GenAIDriftMap),
}

pub enum DriftConfig {
    Spc(Arc<RwLock<SpcDriftConfig>>),
    Psi(Arc<RwLock<PsiDriftConfig>>),
    GenAI(GenAIDriftConfig),
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

    pub fn custom_config(&self) -> Result<CustomMetricDriftConfig, DriftError> {
        match self {
            DriftConfig::Custom(cfg) => Ok(cfg.clone()),
            _ => Err(DriftError::InvalidConfigError),
        }
    }

    pub fn genai_config(&self) -> Result<GenAIDriftConfig, DriftError> {
        match self {
            DriftConfig::GenAI(cfg) => Ok(cfg.clone()),
            _ => Err(DriftError::InvalidConfigError),
        }
    }
}

pub enum Drifter {
    Spc(SpcDrifter),
    Psi(PsiDrifter),
    Custom(CustomDrifter),
    GenAI(ClientGenAIDrifter),
}

impl Debug for Drifter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Drifter::Spc(_) => write!(f, "SpcDrifter"),
            Drifter::Psi(_) => write!(f, "PsiDrifter"),
            Drifter::Custom(_) => write!(f, "CustomDrifter"),
            Drifter::GenAI(_) => write!(f, "GenAIDrifter"),
        }
    }
}

impl Drifter {
    fn from_drift_type(drift_type: DriftType) -> Result<Self, DriftError> {
        match drift_type {
            DriftType::Spc => Ok(Drifter::Spc(SpcDrifter::new())),
            DriftType::Psi => Ok(Drifter::Psi(PsiDrifter::new())),
            DriftType::Custom => Ok(Drifter::Custom(CustomDrifter::new())),
            DriftType::GenAI => Ok(Drifter::GenAI(ClientGenAIDrifter::new())),
        }
    }

    fn create_drift_profile<'py>(
        &mut self,
        py: Python<'py>,
        data: &Bound<'py, PyAny>,
        data_type: &DataType,
        config: DriftConfig,
        workflow: Option<Bound<'py, PyAny>>,
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
            Drifter::GenAI(drifter) => {
                // GenAI drift profiles are created separately, so we will handle this in the create_genai_drift_profile method
                let metrics = if data.is_instance_of::<PyList>() {
                    data.extract::<Vec<GenAIDriftMetric>>()?
                } else {
                    let metric = data.extract::<GenAIDriftMetric>()?;
                    vec![metric]
                };
                let profile =
                    drifter.create_drift_profile(config.genai_config()?, metrics, workflow)?;
                Ok(DriftProfile::GenAI(profile))
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

            Drifter::GenAI(drifter) => {
                // extract data to be Vec<GenAIRecord>
                let data = if data.is_instance_of::<PyList>() {
                    data.extract::<Vec<GenAIRecord>>()?
                } else {
                    let metric = data.extract::<GenAIRecord>()?;
                    vec![metric]
                };
                let records = drifter.compute_drift(data, profile.get_genai_profile()?)?;

                Ok(DriftMap::GenAI(GenAIDriftMap { records }))
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
    /// The data can be a numpy array, pandas dataframe, or pyarrow table, Vec<CustomMetric>, or Vec<GenAIDriftMetric>
    /// ## Arguments:
    /// - `data`: The data to create the drift profile from. This can be a numpy array, pandas dataframe, pyarrow table, Vec<CustomMetric>, or Vec<GenAIDriftMetric>.
    /// - `config`: The configuration for the drift profile. This is optional and if not provided, a default configuration will be created based on the drift type.
    /// - `data_type`: The type of the data. This is optional and if not provided, it will be inferred from the data class name.
    /// - `workflow`: An optional workflow to be used with the drift profile. This is only applicable for GenAI drift profiles.
    #[pyo3(signature = (data, config=None, data_type=None, workflow=None))]
    pub fn create_drift_profile<'py>(
        &self,
        py: Python<'py>,
        data: &Bound<'py, PyAny>,
        config: Option<&Bound<'py, PyAny>>,
        data_type: Option<&DataType>,
        workflow: Option<Bound<'py, PyAny>>,
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
                DriftType::GenAI => {
                    let config = obj.extract::<GenAIDriftConfig>()?;
                    DriftConfig::GenAI(config)
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
            None => {
                let class = data.getattr("__class__")?;
                let module = class.getattr("__module__")?.str()?.to_string();
                let name = class.getattr("__name__")?.str()?.to_string();
                let full_class_name = format!("{module}.{name}");

                &DataType::from_module_name(&full_class_name).unwrap_or(DataType::Unknown)
                // for handling custom
            }
        };

        let profile =
            drift_helper.create_drift_profile(py, data, data_type, config_helper, workflow)?;

        match profile {
            DriftProfile::Spc(profile) => Ok(profile.into_bound_py_any(py)?),
            DriftProfile::Psi(profile) => Ok(profile.into_bound_py_any(py)?),
            DriftProfile::Custom(profile) => Ok(profile.into_bound_py_any(py)?),
            DriftProfile::GenAI(profile) => Ok(profile.into_bound_py_any(py)?),
        }
    }

    // Specific method for creating GenAI drift profiles
    // This is to avoid confusion with the other drifters
    #[pyo3(signature = (config, assertions=None, llm_judge_tasks=None))]
    pub fn create_genai_drift_profile<'py>(
        &mut self,
        py: Python<'py>,
        config: GenAIDriftConfig,
        assertions: Option<Vec<AssertionTask>>,
        llm_judge_tasks: Option<Vec<LLMJudgeTask>>,
    ) -> Result<Bound<'py, PyAny>, DriftError> {
        let profile = GenAIEvalProfile::new(config, assertions, llm_judge_tasks)?;
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
            DriftType::GenAI => {
                let profile = drift_profile.extract::<GenAIEvalProfile>()?;
                DriftProfile::GenAI(profile)
            }
        };

        // if data_type is None, try to infer it from the class name
        // This is for handling, numpy, pandas, pyarrow
        // skip if drift_type is LLM, as it will be handled separately

        let data_type = match data_type {
            Some(data_type) => data_type,
            None => {
                if drift_type == DriftType::GenAI {
                    // For GenAI, we will handle it separately in the create_genai_drift_profile method
                    &DataType::GenAI
                } else {
                    let class = data.getattr("__class__")?;
                    let module = class.getattr("__module__")?.str()?.to_string();
                    let name = class.getattr("__name__")?.str()?.to_string();
                    let full_class_name = format!("{module}.{name}");

                    // for handling custom
                    &DataType::from_module_name(&full_class_name).unwrap_or(DataType::Unknown)
                }
            }
        };

        let mut drift_helper = Drifter::from_drift_type(drift_type)?;

        let drift_map = drift_helper.compute_drift(py, data, data_type, &profile)?;

        match drift_map {
            DriftMap::Spc(map) => Ok(map.into_bound_py_any(py)?),
            DriftMap::Psi(map) => Ok(map.into_bound_py_any(py)?),
            DriftMap::GenAI(map) => Ok(map.into_bound_py_any(py)?),
        }
    }
}

impl PyDrifter {
    /// Reproduction of `create_genai_drift_profile` but allows for passing a runtime
    /// This is used in opsml to allow passing the Opsml runtime
    pub fn create_genai_drift_profile_with_runtime<'py>(
        &mut self,
        py: Python<'py>,
        config: GenAIDriftConfig,
        metrics: Vec<GenAIDriftMetric>,
        workflow: Option<Bound<'py, PyAny>>,
        runtime: Arc<tokio::runtime::Runtime>,
    ) -> Result<Bound<'py, PyAny>, DriftError> {
        let profile = GenAIEvalProfile::new_with_runtime(config, metrics, workflow, runtime)?;
        Ok(profile.into_bound_py_any(py)?)
    }
}
