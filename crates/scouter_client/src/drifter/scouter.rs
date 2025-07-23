use crate::data_utils::DataConverterEnum;
use crate::drifter::{custom::CustomDrifter, llm::LLMDrifter, psi::PsiDrifter, spc::SpcDrifter};
use pyo3::prelude::*;
use pyo3::types::PyList;
use pyo3::IntoPyObjectExt;
use scouter_drift::error::DriftError;
use scouter_drift::spc::SpcDriftMap;
use scouter_types::llm::LLMMetric;
use scouter_types::spc::SpcDriftProfile;
use scouter_types::{
    custom::{CustomDriftProfile, CustomMetric, CustomMetricDriftConfig},
    llm::{LLMDriftConfig, LLMDriftProfile},
    psi::{PsiDriftConfig, PsiDriftMap, PsiDriftProfile},
    spc::SpcDriftConfig,
    DataType, DriftProfile, DriftType,
};
use std::sync::Arc;
use std::sync::RwLock;
pub enum DriftMap {
    Spc(SpcDriftMap),
    Psi(PsiDriftMap),
}

pub enum DriftConfig {
    Spc(Arc<RwLock<SpcDriftConfig>>),
    Psi(Arc<RwLock<PsiDriftConfig>>),
    LLM(Arc<RwLock<LLMDriftConfig>>),
    Custom(Arc<RwLock<CustomMetricDriftConfig>>),
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

    pub fn custom_config(&self) -> Result<Arc<RwLock<CustomMetricDriftConfig>>, DriftError> {
        match self {
            DriftConfig::Custom(cfg) => Ok(cfg.clone()),
            _ => Err(DriftError::InvalidConfigError),
        }
    }

    pub fn llm_config(&self) -> Result<Arc<RwLock<LLMDriftConfig>>, DriftError> {
        match self {
            DriftConfig::LLM(cfg) => Ok(cfg.clone()),
            _ => Err(DriftError::InvalidConfigError),
        }
    }
}

pub enum Drifter {
    Spc(SpcDrifter),
    Psi(PsiDrifter),
    Custom(CustomDrifter),
    LLM(LLMDrifter),
}

impl Drifter {
    fn from_drift_type(drift_type: DriftType) -> Result<Self, DriftError> {
        match drift_type {
            DriftType::Spc => Ok(Drifter::Spc(SpcDrifter::new())),
            DriftType::Psi => Ok(Drifter::Psi(PsiDrifter::new())),
            DriftType::Custom => Ok(Drifter::Custom(CustomDrifter::new())),
            _ => {
                // For LLM, we will handle it separately in the create_llm_drift_profile method
                // This is to avoid confusion with the other drifters
                Err(DriftError::WrongMethodError(
                    "LLMDrifter should be handled separately. Use create_llm_drift_profile instead.".to_string(),
                ))
            }
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

                let profile = drifter.create_drift_profile(
                    config.custom_config()?.read().unwrap().clone(),
                    data,
                    None,
                )?;
                Ok(DriftProfile::Custom(profile))
            }
            Drifter::LLM(drifter) => {
                // LLM drift profiles are created separately, so we will handle this in the create_llm_drift_profile method
                let metrics = if data.is_instance_of::<PyList>() {
                    data.extract::<Vec<LLMMetric>>()?
                } else {
                    let metric = data.extract::<LLMMetric>()?;
                    vec![metric]
                };
                let profile = drifter.create_drift_profile(
                    config.llm_config()?.read().unwrap().clone(),
                    metrics,
                    workflow,
                )?;
                Ok(DriftProfile::LLM(profile))
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

            Drifter::LLM(_) => {
                // check if data is pylist. If it is, convert to Vec<CustomMetric>
                // if not extract to CustomMetric and add to vec
                Err(DriftError::NotImplemented)
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
    /// The data can be a numpy array, pandas dataframe, or pyarrow table, Vec<CustomMetric>, or Vec<LLMMetric>
    /// ## Arguments:
    /// - `data`: The data to create the drift profile from. This can be a numpy array, pandas dataframe, pyarrow table, Vec<CustomMetric>, or Vec<LLMMetric>.
    /// - `config`: The configuration for the drift profile. This is optional and if not provided, a default configuration will be created based on the drift type.
    /// - `data_type`: The type of the data. This is optional and if not provided, it will be inferred from the data class name.
    /// - `workflow`: An optional workflow to be used with the drift profile. This is only applicable for LLM drift profiles.
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

        let (config_helper, drift_type) = if config.is_some() {
            let obj = config.unwrap();
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
                    DriftConfig::Custom(Arc::new(config.into()))
                }
                DriftType::LLM => {
                    let config = obj.extract::<LLMDriftConfig>()?;
                    DriftConfig::LLM(Arc::new(config.into()))
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
            DriftProfile::LLM(profile) => Ok(profile.into_bound_py_any(py)?),
        }
    }

    // Specific method for creating LLM drift profiles
    // This is to avoid confusion with the other drifters
    #[pyo3(signature = (config, metrics, workflow=None))]
    pub fn create_llm_drift_profile<'py>(
        &mut self,
        py: Python<'py>,
        config: LLMDriftConfig,
        metrics: Vec<LLMMetric>,
        workflow: Option<Bound<'py, PyAny>>,
    ) -> Result<Bound<'py, PyAny>, DriftError> {
        let profile = LLMDriftProfile::new(config, metrics, workflow)?;
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
            DriftType::LLM => {
                let profile = drift_profile.extract::<LLMDriftProfile>()?;
                DriftProfile::LLM(profile)
            }
        };

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

        let mut drift_helper = Drifter::from_drift_type(drift_type)?;

        let drift_map = drift_helper.compute_drift(py, data, data_type, &profile)?;

        match drift_map {
            DriftMap::Spc(map) => Ok(map.into_bound_py_any(py)?),
            DriftMap::Psi(map) => Ok(map.into_bound_py_any(py)?),
        }
    }
}
