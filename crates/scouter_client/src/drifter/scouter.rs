use std::collections::HashMap;

use crate::data_utils::ConvertedData;
use crate::data_utils::{convert_array_type, DataConverterEnum};
use numpy::{PyArray2, PyArrayMethods};
use numpy::PyReadonlyArray2;
use numpy::ToPyArray;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use scouter_drift::{
    psi::PsiMonitor,
    spc::{generate_alerts, SpcDriftMap, SpcMonitor},
    CategoricalFeatureHelpers,
};
use scouter_error::{PyScouterError, ScouterError};
use scouter_types::DriftProfile;
use scouter_types::{
    create_feature_map,
    custom::{CustomDriftProfile, CustomMetric, CustomMetricDriftConfig},
    psi::{PsiDriftConfig, PsiDriftMap, PsiDriftProfile},
    spc::{SpcAlertRule, SpcDriftConfig, SpcDriftProfile, SpcFeatureAlerts},
    DataType, DriftType, ServerRecords,
};
use num_traits::{Float, FromPrimitive, Num};
use std::fmt::Debug;

#[pyclass]
pub struct SpcDrifter {
    monitor: SpcMonitor,
}


#[pymethods]
#[allow(clippy::new_without_default)]
impl SpcDrifter {
    #[new]
    pub fn new() -> Self {
        //maybe create different drifters based on type of monitoring?
        Self {
            monitor: SpcMonitor::new(),
        }
    }

    pub fn generate_alerts(
        &mut self,
        drift_array: PyReadonlyArray2<f64>,
        features: Vec<String>,
        alert_rule: SpcAlertRule,
    ) -> PyResult<SpcFeatureAlerts> {
        let drift_array = drift_array.as_array();

        let alerts = match generate_alerts(&drift_array, &features, &alert_rule) {
            Ok(alerts) => alerts,
            Err(_e) => {
                return Err(PyValueError::new_err("Failed to generate alerts"));
            }
        };

        Ok(alerts)
    }

    pub fn sample_data_f32(
        &mut self,
        array: PyReadonlyArray2<f32>,
        features: Vec<String>,
        drift_profile: SpcDriftProfile,
    ) -> PyResult<ServerRecords> {
        let array = array.as_array();

        let records = match self.monitor.sample_data(&features, &array, &drift_profile) {
            Ok(profile) => profile,
            Err(_e) => {
                return Err(PyValueError::new_err("Failed to sample data"));
            }
        };

        Ok(records)
    }

    pub fn sample_data_f64(
        &mut self,
        array: PyReadonlyArray2<f64>,
        features: Vec<String>,
        drift_profile: SpcDriftProfile,
    ) -> PyResult<ServerRecords> {
        let array = array.as_array();

        let records = match self.monitor.sample_data(&features, &array, &drift_profile) {
            Ok(profile) => profile,
            Err(_e) => {
                return Err(PyValueError::new_err("Failed to sample data"));
            }
        };

        Ok(records)
    }


}

impl SpcDrifter {

    fn create_string_drift_profile(
        &mut self,
        array: Vec<Vec<String>>,
        features: Vec<String>,
        mut drift_config: SpcDriftConfig,
    ) -> Result<SpcDriftProfile, ScouterError> {
        let feature_map = match create_feature_map(&features, &array) {
            Ok(feature_map) => feature_map,
            Err(_e) => {
                let msg = format!("Failed to create feature map: {}", _e);
                return Err(ScouterError::Error(msg));
            }
        };

        drift_config.update_feature_map(feature_map.clone());

        let array =
            match self
                .monitor
                .convert_strings_to_ndarray_f32(&features, &array, &feature_map)
            {
                Ok(array) => array,
                Err(_e) => {
                    return Err(ScouterError::Error("Failed to create 2D drift profile".to_string()));
                }
            };

        let profile =
            match self
                .monitor
                .create_2d_drift_profile(&features, &array.view(), &drift_config)
            {
                Ok(profile) => profile,
                Err(_e) => {
                    return Err(ScouterError::Error("Failed to create 2D drift profile".to_string()));
                }
            };

        Ok(profile)
    }

    fn create_numeric_drift_profile<F>(
        &mut self,
        array: PyReadonlyArray2<F>,
        features: Vec<String>,
        drift_config: SpcDriftConfig,
    ) -> Result<SpcDriftProfile, ScouterError> 
    
    where
        F: Float
            + Sync
            + FromPrimitive
            + Send
            + Num
            + Debug
            + num_traits::Zero
            + ndarray::ScalarOperand
            + numpy::Element,
        F: Into<f64>,
    {
        let array = array.as_array();

        let profile = self
            .monitor
            .create_2d_drift_profile(&features, &array, &drift_config)?;

        Ok(profile)
    }

    pub fn compute_drift<F>(
        &mut self,
        array: PyReadonlyArray2<F>,
        features: Vec<String>,
        drift_profile: SpcDriftProfile,
    ) -> PyResult<SpcDriftMap> 

    where
        F: Float
            + Sync
            + FromPrimitive
            + Send
            + Num
            + Debug
            + num_traits::Zero
            + ndarray::ScalarOperand
            + numpy::Element,
        F: Into<f64>,
    
    {
        let drift_map =
            match self
                .monitor
                .compute_drift(&features, &array.as_array(), &drift_profile)
            {
                Ok(drift_map) => drift_map,
                Err(_e) => {
                    return Err(PyValueError::new_err("Failed to compute drift"));
                }
            };

        Ok(drift_map)
    }
}

#[pyclass]
pub struct PsiDrifter {
    monitor: PsiMonitor,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl PsiDrifter {
    #[new]
    pub fn new() -> Self {
        Self {
            monitor: PsiMonitor::default(),
        }
    }
    pub fn create_string_drift_profile(
        &mut self,
        array: Vec<Vec<String>>,
        features: Vec<String>,
        mut drift_config: PsiDriftConfig,
    ) -> PyResult<PsiDriftProfile> {
        let feature_map = match self.monitor.create_feature_map(&features, &array) {
            Ok(feature_map) => feature_map,
            Err(_e) => {
                let msg = format!("Failed to create feature map: {}", _e);
                return Err(PyValueError::new_err(msg));
            }
        };

        drift_config.update_feature_map(feature_map.clone());

        let array =
            match self
                .monitor
                .convert_strings_to_ndarray_f32(&features, &array, &feature_map)
            {
                Ok(array) => array,
                Err(_e) => {
                    return Err(PyValueError::new_err("Failed to create 2D monitor profile"));
                }
            };

        let profile =
            match self
                .monitor
                .create_2d_drift_profile(&features, &array.view(), &drift_config)
            {
                Ok(profile) => profile,
                Err(_e) => {
                    return Err(PyValueError::new_err("Failed to create 2D monitor profile"));
                }
            };

        Ok(profile)
    }

   
    pub fn compute_drift_f32(
        &mut self,
        array: PyReadonlyArray2<f32>,
        features: Vec<String>,
        drift_profile: PsiDriftProfile,
    ) -> PyResult<PsiDriftMap> {
        let drift_map =
            match self
                .monitor
                .compute_drift(&features, &array.as_array(), &drift_profile)
            {
                Ok(drift_map) => drift_map,
                Err(_e) => {
                    return Err(PyValueError::new_err("Failed to compute drift"));
                }
            };

        Ok(drift_map)
    }

    pub fn compute_drift_f64(
        &mut self,
        array: PyReadonlyArray2<f64>,
        features: Vec<String>,
        drift_profile: PsiDriftProfile,
    ) -> PyResult<PsiDriftMap> {
        let drift_map =
            match self
                .monitor
                .compute_drift(&features, &array.as_array(), &drift_profile)
            {
                Ok(drift_map) => drift_map,
                Err(_e) => {
                    return Err(PyValueError::new_err("Failed to compute drift"));
                }
            };

        Ok(drift_map)
    }
}

impl PsiDrifter {

    pub fn create_numeric_drift_profile<F>(
        &mut self,
        array: PyReadonlyArray2<F>,
        features: Vec<String>,
        drift_config: PsiDriftConfig,
    ) -> Result<PsiDriftProfile, ScouterError> 
    
    where
        F: Float + Sync + FromPrimitive + Default,
        F: Into<f64>,
        F: numpy::Element,

    {
        let array = array.as_array();

        let profile = self
            .monitor
            .create_2d_drift_profile(&features, &array, &drift_config)?;
        
        Ok(profile)
    }
}

#[pyclass]
pub struct CustomDrifter {}

#[pymethods]
#[allow(clippy::new_without_default)]
impl CustomDrifter {
    #[new]
    pub fn new() -> Self {
        Self {}
    }

    #[pyo3(signature = (config, comparison_metrics, scouter_version=None))]
    pub fn create_drift_profile(
        &mut self,
        config: CustomMetricDriftConfig,
        comparison_metrics: Vec<CustomMetric>,
        scouter_version: Option<String>,
    ) -> PyResult<CustomDriftProfile> {
        Ok(CustomDriftProfile::new(
            config,
            comparison_metrics,
            scouter_version,
        )?)
    }
}


pub enum DriftConfig {
    Spc(SpcDriftConfig),
    Psi(PsiDriftConfig),
    Custom(CustomMetricDriftConfig),
}

impl DriftConfig {

    pub fn spc_config(&self) -> Result<&SpcDriftConfig, ScouterError> {
        match self {
            DriftConfig::Spc(cfg) => Ok(cfg),
            _ => Err(ScouterError::Error("Invalid config type for SpcDrifter".to_string())),
        }
    }
}

pub enum DriftHelper {
    Spc(SpcDrifter),
    Psi(PsiDrifter),
    Custom(CustomDrifter),
}

impl DriftHelper {
    fn from_drift_type(drift_type: DriftType) -> Self {
        match drift_type {
            DriftType::Spc => DriftHelper::Spc(SpcDrifter::new()),
            DriftType::Psi => DriftHelper::Psi(PsiDrifter::new()),
            DriftType::Custom => DriftHelper::Custom(CustomDrifter::new()),
        }
    }

    fn create_drift_profile<'py, F: numpy::Element>(
        &mut self,
        data: ConvertedData<'py>,
        config: DriftConfig,
    ) -> Result<DriftProfile, ScouterError> 
    where
        F: Float + Sync + FromPrimitive + Default  + std::fmt::Debug + ndarray::ScalarOperand,
        F: Into<f64>,
    {
        let (num_features, num_array, dtype, string_features, string_array) = data;

        match self {
            DriftHelper::Spc(drifter) => {
                let mut features = HashMap::new();
                let cfg = config.spc_config()?;
                let mut final_config = cfg.clone();

                if let Some(string_array) = string_array {
                    let profile = drifter.create_string_drift_profile(string_array, string_features, final_config.clone())?;
                    final_config.feature_map = profile.config.feature_map.clone();
                    features.extend(profile.features);
                }

                if let Some(num_array) = num_array {
                    let array = num_array.downcast_into::<PyArray2<F>>().map_err(|e| ScouterError::Error(e.to_string()))?.readonly();
                    let profile = drifter.create_numeric_drift_profile(array, num_features, final_config.clone())?;
                    features.extend(profile.features);
                }

                Ok(DriftProfile::SpcDriftProfile(SpcDriftProfile::new(features, final_config, None)))
            }
            DriftHelper::Psi(drifter) => {
                let mut features = HashMap::new();
                let mut final_config = if let DriftConfig::Psi(cfg) = config {
                    cfg
                } else {
                    return Err(ScouterError::Error("Invalid config type for PsiDrifter".to_string()));
                };

                if let Some(string_array) = string_array {
                    let profile = drifter.create_string_drift_profile(string_array, string_features, final_config.clone())?;
                    final_config.feature_map = profile.config.feature_map.clone();
                    features.extend(profile.features);
                }

                if let Some(num_array) = num_array {
                    let array = num_array.downcast_into::<PyArray2<F>>().map_err(|e| ScouterError::Error(e.to_string()))?.readonly();
                    let profile = drifter.create_numeric_drift_profile(array, num_features, final_config)?
                    features.extend(profile.features);
                }

                Ok(DriftProfile::PsiDriftProfile(PsiDriftProfile::new(features, final_config, None)))
            }
            DriftHelper::Custom(drifter) => {
                let final_config = if let DriftConfig::Custom(cfg) = config {
                    cfg
                } else {
                    return Err(ScouterError::Error("Invalid config type for CustomDrifter".to_string()));
                };

                let profile = drifter.create_drift_profile(final_config, vec![], None)?;
                Ok(DriftProfile::CustomDriftProfile(profile))
            }
        }
    }
}

#[pyclass(name = "Drifter")]
pub struct PyDrifter {
    drift_helper: DriftHelper,
}

#[pymethods]
impl PyDrifter {
    #[new]
    #[pyo3(signature = (drift_type=None))]
    pub fn new(drift_type: Option<DriftType>) -> Self {
        let drift_type = drift_type.unwrap_or(DriftType::Spc);
        Self {
            drift_helper: DriftHelper::from_drift_type(drift_type),
        }
    }

    #[pyo3(signature = (data, config=None, data_type=None))]
    pub fn create_drift_profile(
        &mut self,
        py: Python,
        data: &Bound<'_, PyAny>,
        config: Option<&Bound<'_, PyAny>>,
        data_type: Option<&DataType>,
    ) -> PyResult<()> {
        // if config is None, then we need to create a default config
        let config_helper = if config.is_some() {
            let obj = config.unwrap();
            let drift_type = obj.getattr("drift_type")?.extract::<DriftType>()?;
            let drift_config = match drift_type {
                DriftType::Spc => {
                    let config = obj.extract::<SpcDriftConfig>()?;
                    DriftConfigHelper::Spc(config)
                }
                DriftType::Psi => {
                    let config = obj.extract::<PsiDriftConfig>()?;
                    DriftConfigHelper::Psi(config)
                }
                DriftType::Custom => {
                    let config = obj.extract::<CustomMetricDriftConfig>()?;
                    DriftConfigHelper::Custom(config)
                }
            };
            drift_config
        } else {
            DriftConfigHelper::Spc(SpcDriftConfig::default())
        };

        // if data_type is None, try to infer it from the class name
        let data_type = match data_type {
            Some(data_type) => data_type,
            None => {
                let class = data.getattr("__class__")?;
                let module = class.getattr("__module__")?.str()?.to_string();
                let name = class.getattr("__name__")?.str()?.to_string();
                let full_class_name = format!("{}.{}", module, name);

                &DataType::from_module_name(&full_class_name)?
            }
        };

        let (num_features, num_array, dtype, string_features, string_vec) =
            DataConverterEnum::convert_data(data_type, data)?;

        // if num_features is not empty, check dtype. If dtype == "float64", process as f64, else process as f32
        DriftHelper::create_spc_drit_profile(&mut self, data, config)

        // convert data to ndarray
        Ok(())
    }
}
