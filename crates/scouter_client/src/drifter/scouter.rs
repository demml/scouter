#![allow(clippy::useless_conversion)]
use crate::data_utils::DataConverterEnum;
use crate::drifter::{custom::CustomDrifter, psi::PsiDrifter, spc::SpcDrifter};
use pyo3::prelude::*;
use pyo3::types::PyList;
use pyo3::IntoPyObjectExt;
use scouter_drift::error::DriftError;
use scouter_drift::spc::SpcDriftMap;
use scouter_types::spc::SpcDriftProfile;
use scouter_types::{
    custom::{CustomDriftProfile, CustomMetric, CustomMetricDriftConfig},
    psi::{PsiDriftConfig, PsiDriftMap, PsiDriftProfile},
    spc::SpcDriftConfig,
    DataType, DriftProfile, DriftType,
};

pub enum DriftMap {
    Spc(SpcDriftMap),
    Psi(PsiDriftMap),
}

pub enum DriftConfig {
    Spc(SpcDriftConfig),
    Psi(PsiDriftConfig),
    Custom(CustomMetricDriftConfig),
}

impl DriftConfig {
    pub fn spc_config(&self) -> Result<&SpcDriftConfig, DriftError> {
        match self {
            DriftConfig::Spc(cfg) => Ok(cfg),
            _ => Err(DriftError::InvalidConfigError),
        }
    }

    pub fn psi_config(&self) -> Result<&PsiDriftConfig, DriftError> {
        match self {
            DriftConfig::Psi(cfg) => Ok(cfg),
            _ => Err(DriftError::InvalidConfigError),
        }
    }

    pub fn custom_config(&self) -> Result<&CustomMetricDriftConfig, DriftError> {
        match self {
            DriftConfig::Custom(cfg) => Ok(cfg),
            _ => Err(DriftError::InvalidConfigError),
        }
    }
}

pub enum Drifter {
    Spc(SpcDrifter),
    Psi(PsiDrifter),
    Custom(CustomDrifter),
}

impl Drifter {
    fn from_drift_type(drift_type: DriftType) -> Self {
        match drift_type {
            DriftType::Spc => Drifter::Spc(SpcDrifter::new()),
            DriftType::Psi => Drifter::Psi(PsiDrifter::new()),
            DriftType::Custom => Drifter::Custom(CustomDrifter::new()),
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
            Drifter::Spc(drifter) => {
                let data = DataConverterEnum::convert_data(py, data_type, data)?;
                let profile = drifter.create_drift_profile(data, config.spc_config()?.clone())?;
                Ok(DriftProfile::Spc(profile))
            }
            Drifter::Psi(drifter) => {
                let data = DataConverterEnum::convert_data(py, data_type, data)?;
                let profile = drifter.create_drift_profile(data, config.psi_config()?.clone())?;
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

                let profile =
                    drifter.create_drift_profile(config.custom_config()?.clone(), data, None)?;
                Ok(DriftProfile::Custom(profile))
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

    #[pyo3(signature = (data, config=None, data_type=None))]
    pub fn create_drift_profile<'py>(
        &self,
        py: Python<'py>,
        data: &Bound<'py, PyAny>,
        config: Option<&Bound<'py, PyAny>>,
        data_type: Option<&DataType>,
    ) -> Result<Bound<'py, PyAny>, DriftError> {
        // if config is None, then we need to create a default config

        let (config_helper, drift_type) = if config.is_some() {
            let obj = config.unwrap();
            let drift_type = obj.getattr("drift_type")?.extract::<DriftType>()?;
            let drift_config = match drift_type {
                DriftType::Spc => {
                    let config = obj.extract::<SpcDriftConfig>()?;
                    DriftConfig::Spc(config)
                }
                DriftType::Psi => {
                    let config = obj.extract::<PsiDriftConfig>()?;
                    DriftConfig::Psi(config)
                }
                DriftType::Custom => {
                    let config = obj.extract::<CustomMetricDriftConfig>()?;
                    DriftConfig::Custom(config)
                }
            };
            (drift_config, drift_type)
        } else {
            (DriftConfig::Spc(SpcDriftConfig::default()), DriftType::Spc)
        };

        let mut drift_helper = Drifter::from_drift_type(drift_type);

        // if data_type is None, try to infer it from the class name
        // This is for handling, numpy, pandas, pyarrow
        let data_type = match data_type {
            Some(data_type) => data_type,
            None => {
                let class = data.getattr("__class__")?;
                let module = class.getattr("__module__")?.str()?.to_string();
                let name = class.getattr("__name__")?.str()?.to_string();
                let full_class_name = format!("{}.{}", module, name);

                &DataType::from_module_name(&full_class_name).unwrap_or(DataType::Unknown)
                // for handling custom
            }
        };

        let profile = drift_helper.create_drift_profile(py, data, data_type, config_helper)?;

        match profile {
            DriftProfile::Spc(profile) => Ok(profile.into_bound_py_any(py)?),
            DriftProfile::Psi(profile) => Ok(profile.into_bound_py_any(py)?),
            DriftProfile::Custom(profile) => Ok(profile.into_bound_py_any(py)?),
        }
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
        };

        // if data_type is None, try to infer it from the class name
        // This is for handling, numpy, pandas, pyarrow
        let data_type = match data_type {
            Some(data_type) => data_type,
            None => {
                let class = data.getattr("__class__")?;
                let module = class.getattr("__module__")?.str()?.to_string();
                let name = class.getattr("__name__")?.str()?.to_string();
                let full_class_name = format!("{}.{}", module, name);

                &DataType::from_module_name(&full_class_name).unwrap_or(DataType::Unknown)
                // for handling custom
            }
        };

        let mut drift_helper = Drifter::from_drift_type(drift_type);

        let drift_map = drift_helper.compute_drift(py, data, data_type, &profile)?;

        match drift_map {
            DriftMap::Spc(map) => Ok(map.into_bound_py_any(py)?),
            DriftMap::Psi(map) => Ok(map.into_bound_py_any(py)?),
        }
    }
}

impl PyDrifter {
    // method used internally to return DriftProfile Enum
    pub fn internal_create_drift_profile<'py>(
        &self,
        py: Python,
        data: &Bound<'py, PyAny>,
        config: Option<&Bound<'py, PyAny>>,
        data_type: Option<&DataType>,
    ) -> Result<DriftProfile, DriftError> {
        // if config is None, then we need to create a default config

        let (config_helper, drift_type) = if config.is_some() {
            let obj = config.unwrap();
            let drift_type = obj.getattr("drift_type")?.extract::<DriftType>()?;
            let drift_config = match drift_type {
                DriftType::Spc => {
                    let config = obj.extract::<SpcDriftConfig>()?;
                    DriftConfig::Spc(config)
                }
                DriftType::Psi => {
                    let config = obj.extract::<PsiDriftConfig>()?;
                    DriftConfig::Psi(config)
                }
                DriftType::Custom => {
                    let config = obj.extract::<CustomMetricDriftConfig>()?;
                    DriftConfig::Custom(config)
                }
            };
            (drift_config, drift_type)
        } else {
            (DriftConfig::Spc(SpcDriftConfig::default()), DriftType::Spc)
        };

        let mut drift_helper = Drifter::from_drift_type(drift_type);

        // if data_type is None, try to infer it from the class name
        // This is for handling, numpy, pandas, pyarrow
        let data_type = match data_type {
            Some(data_type) => data_type,
            None => {
                let class = data.getattr("__class__")?;
                let module = class.getattr("__module__")?.str()?.to_string();
                let name = class.getattr("__name__")?.str()?.to_string();
                let full_class_name = format!("{}.{}", module, name);

                &DataType::from_module_name(&full_class_name).unwrap_or(DataType::Unknown)
                // for handling custom
            }
        };

        drift_helper.create_drift_profile(py, data, data_type, config_helper)
    }
}
