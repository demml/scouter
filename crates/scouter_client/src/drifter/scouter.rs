use crate::data_utils::DataConverterEnum;
use crate::drifter::{custom::CustomDrifter, psi::PsiDrifter, spc::SpcDrifter};
use pyo3::prelude::*;
use pyo3::types::PyList;
use scouter_error::{PyScouterError, ScouterError};
use scouter_types::custom::CustomMetric;
use scouter_types::DataType;
use scouter_types::DriftProfile;
use scouter_types::{
    custom::CustomMetricDriftConfig, psi::PsiDriftConfig, spc::SpcDriftConfig, DriftType,
};

pub enum DriftConfig {
    Spc(SpcDriftConfig),
    Psi(PsiDriftConfig),
    Custom(CustomMetricDriftConfig),
}

impl DriftConfig {
    pub fn spc_config(&self) -> Result<&SpcDriftConfig, ScouterError> {
        match self {
            DriftConfig::Spc(cfg) => Ok(cfg),
            _ => Err(ScouterError::Error(
                "Invalid config type for SpcDrifter".to_string(),
            )),
        }
    }

    pub fn psi_config(&self) -> Result<&PsiDriftConfig, ScouterError> {
        match self {
            DriftConfig::Psi(cfg) => Ok(cfg),
            _ => Err(ScouterError::Error(
                "Invalid config type for PsiDrifter".to_string(),
            )),
        }
    }

    pub fn custom_config(&self) -> Result<&CustomMetricDriftConfig, ScouterError> {
        match self {
            DriftConfig::Custom(cfg) => Ok(cfg),
            _ => Err(ScouterError::Error(
                "Invalid config type for CustomDrifter".to_string(),
            )),
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
    ) -> Result<DriftProfile, ScouterError> {
        match self {
            Drifter::Spc(drifter) => {
                let data = DataConverterEnum::convert_data(py, data_type, data)?;
                let profile = drifter.create_drift_profile(data, config.spc_config()?.clone())?;
                Ok(DriftProfile::SpcDriftProfile(profile))
            }
            Drifter::Psi(drifter) => {
                let data = DataConverterEnum::convert_data(py, data_type, data)?;
                let profile = drifter.create_drift_profile(data, config.psi_config()?.clone())?;
                Ok(DriftProfile::PsiDriftProfile(profile))
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
                Ok(DriftProfile::CustomDriftProfile(profile))
            }
        }
    }
}

#[pyclass(name = "Drifter")]
pub struct PyDrifter {
    _drifter: Drifter,
}

#[pymethods]
impl PyDrifter {
    #[new]
    #[pyo3(signature = (drift_type=None))]
    pub fn new(drift_type: Option<DriftType>) -> Self {
        let drift_type = drift_type.unwrap_or(DriftType::Spc);
        Self {
            _drifter: Drifter::from_drift_type(drift_type),
        }
    }

    #[pyo3(signature = (data, config=None, data_type=None))]
    pub fn create_drift_profile<'py>(
        &mut self,
        py: Python,
        data: &Bound<'py, PyAny>,
        config: Option<&Bound<'py, PyAny>>,
        data_type: Option<&DataType>,
    ) -> PyResult<DriftProfile> {
        // if config is None, then we need to create a default config
        let config_helper = if config.is_some() {
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
            drift_config
        } else {
            DriftConfig::Spc(SpcDriftConfig::default())
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

        self._drifter
            .create_drift_profile(py, data, data_type, config_helper)
            .map_err(|e| PyScouterError::new_err(e.to_string()))
    }
}
