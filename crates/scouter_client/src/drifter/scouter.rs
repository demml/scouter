use numpy::PyArray2;
use numpy::PyReadonlyArray2;
use numpy::ToPyArray;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use scouter_drift::{
    psi::PsiMonitor,
    spc::{generate_alerts, SpcDriftMap, SpcMonitor},
    CategoricalFeatureHelpers,
};
use scouter_error::ScouterError;
use scouter_types::DriftType;
use scouter_types::{
    create_feature_map,
    custom::{CustomDriftProfile, CustomMetric, CustomMetricDriftConfig},
    psi::{PsiDriftConfig, PsiDriftMap, PsiDriftProfile},
    spc::{SpcAlertRule, SpcDriftConfig, SpcDriftProfile, SpcFeatureAlerts},
    ServerRecords,
};

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

    pub fn convert_strings_to_numpy_f32<'py>(
        &mut self,
        py: Python<'py>,
        features: Vec<String>,
        array: Vec<Vec<String>>,
        drift_profile: SpcDriftProfile,
    ) -> PyResult<pyo3::Bound<'py, PyArray2<f32>>> {
        let array = match self.monitor.convert_strings_to_ndarray_f32(
            &features,
            &array,
            &drift_profile
                .config
                .feature_map
                .ok_or(ScouterError::MissingFeatureMapError)
                .unwrap(),
        ) {
            Ok(array) => array,
            Err(_e) => {
                return Err(PyValueError::new_err(
                    "Failed to convert strings to ndarray",
                ));
            }
        };

        Ok(array.to_pyarray(py))
    }

    pub fn convert_strings_to_numpy_f64<'py>(
        &mut self,
        py: Python<'py>,
        features: Vec<String>,
        array: Vec<Vec<String>>,
        drift_profile: SpcDriftProfile,
    ) -> PyResult<pyo3::Bound<'py, PyArray2<f64>>> {
        let array = match self.monitor.convert_strings_to_ndarray_f64(
            &features,
            &array,
            &drift_profile
                .config
                .feature_map
                .ok_or(ScouterError::MissingFeatureMapError)
                .unwrap(),
        ) {
            Ok(array) => array,
            Err(_e) => {
                return Err(PyValueError::new_err(
                    "Failed to convert strings to ndarray",
                ));
            }
        };

        Ok(array.to_pyarray(py))
    }

    pub fn create_string_drift_profile(
        &mut self,
        array: Vec<Vec<String>>,
        features: Vec<String>,
        mut drift_config: SpcDriftConfig,
    ) -> PyResult<SpcDriftProfile> {
        let feature_map = match create_feature_map(&features, &array) {
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

    pub fn create_numeric_drift_profile_f32(
        &mut self,
        array: PyReadonlyArray2<f32>,
        features: Vec<String>,
        drift_config: SpcDriftConfig,
    ) -> PyResult<SpcDriftProfile> {
        let array = array.as_array();

        let profile = match self
            .monitor
            .create_2d_drift_profile(&features, &array, &drift_config)
        {
            Ok(profile) => profile,
            Err(_e) => {
                return Err(PyValueError::new_err("Failed to create 2D monitor profile"));
            }
        };

        Ok(profile)
    }

    pub fn create_numeric_drift_profile_f64(
        &mut self,
        array: PyReadonlyArray2<f64>,
        features: Vec<String>,
        drift_config: SpcDriftConfig,
    ) -> PyResult<SpcDriftProfile> {
        let array = array.as_array();

        let profile = match self
            .monitor
            .create_2d_drift_profile(&features, &array, &drift_config)
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
        drift_profile: SpcDriftProfile,
    ) -> PyResult<SpcDriftMap> {
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
        drift_profile: SpcDriftProfile,
    ) -> PyResult<SpcDriftMap> {
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

    pub fn create_numeric_drift_profile_f32(
        &mut self,
        array: PyReadonlyArray2<f32>,
        features: Vec<String>,
        drift_config: PsiDriftConfig,
    ) -> PyResult<PsiDriftProfile> {
        let array = array.as_array();

        let profile = match self
            .monitor
            .create_2d_drift_profile(&features, &array, &drift_config)
        {
            Ok(profile) => profile,
            Err(_e) => {
                return Err(PyValueError::new_err("Failed to create 2D monitor profile"));
            }
        };

        Ok(profile)
    }

    pub fn create_numeric_drift_profile_f64(
        &mut self,
        array: PyReadonlyArray2<f64>,
        features: Vec<String>,
        drift_config: PsiDriftConfig,
    ) -> PyResult<PsiDriftProfile> {
        let array = array.as_array();

        let profile = match self
            .monitor
            .create_2d_drift_profile(&features, &array, &drift_config)
        {
            Ok(profile) => profile,
            Err(_e) => {
                return Err(PyValueError::new_err("Failed to create 2D monitor profile"));
            }
        };

        Ok(profile)
    }

    pub fn convert_strings_to_numpy_f32<'py>(
        &mut self,
        py: Python<'py>,
        features: Vec<String>,
        array: Vec<Vec<String>>,
        drift_profile: PsiDriftProfile,
    ) -> PyResult<Bound<'py, PyArray2<f32>>> {
        let array = match self.monitor.convert_strings_to_ndarray_f32(
            &features,
            &array,
            &drift_profile.config.feature_map,
        ) {
            Ok(array) => array,
            Err(_e) => {
                return Err(PyValueError::new_err(
                    "Failed to convert strings to ndarray",
                ));
            }
        };

        Ok(array.to_pyarray(py))
    }

    pub fn convert_strings_to_numpy_f64<'py>(
        &mut self,
        py: Python<'py>,
        features: Vec<String>,
        array: Vec<Vec<String>>,
        drift_profile: PsiDriftProfile,
    ) -> PyResult<Bound<'py, PyArray2<f64>>> {
        let array = match self.monitor.convert_strings_to_ndarray_f64(
            &features,
            &array,
            &drift_profile.config.feature_map,
        ) {
            Ok(array) => array,
            Err(_e) => {
                return Err(PyValueError::new_err(
                    "Failed to convert strings to ndarray",
                ));
            }
        };

        Ok(array.to_pyarray(py))
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
}


#[pyclass(name="Drifter")]
pub struct PyDrifter {
    drift_helper: DriftHelper
}

#[pymethods]
impl PyDrifter {
    #[new]
    #[pyo3(signature = (drift_type=None))]
    pub fn new(drift_type: Option<DriftType>) -> Self {

        let drift_type = drift_type.unwrap_or(DriftType::Spc);
        Self {
            
            drift_helper: DriftHelper::from_drift_type(drift_type)

        }
    }

}
