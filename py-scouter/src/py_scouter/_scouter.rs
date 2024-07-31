use core::{f32, num};
use ndarray::prelude::*;
use ndarray::prelude::*;
use numpy::ndarray::parallel::prelude::IntoParallelRefIterator;
use numpy::ndarray::{ArrayBase, ViewRepr};
use pyo3::types::PyString;
use scouter::core::alert::generate_alerts;
use scouter::core::monitor::Monitor;
use scouter::core::num_profiler::NumProfiler;
use scouter::utils::types::{
    AlertRule, DataProfile, DriftConfig, DriftMap, DriftProfile, DriftServerRecord, FeatureAlerts,
};
use std::string;

use numpy::{PyArray2, PyFixedString, PyReadonlyArray2};
use pyo3::exceptions::PyValueError;

use pyo3::prelude::*;

#[pyclass]
pub struct ScouterProfiler {
    num_profiler: NumProfiler,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl ScouterProfiler {
    #[new]
    pub fn new() -> Self {
        Self {
            num_profiler: NumProfiler::default(),
        }
    }

    pub fn create_data_profile_f32(
        &mut self,
        numeric_array: Option<PyReadonlyArray2<f32>>,
        string_array: Option<ArrayView2<String>>,
        numeric_features: Option<Vec<String>>,
        string_features: Option<Vec<String>>,
        bin_size: Option<usize>,
    ) -> PyResult<DataProfile>
    where
        ArrayView2<String>: PyFunctionArgument<'a, 'py>,
    {
        let mut profiles = vec![];

        if string_features.is_some() && string_array.is_some() {
            let string_features = string_features.unwrap();
            let string_array = string_array.unwrap();

            let array: Array2<String> = Array2::from_shape_vec(
                (string_array.len(), string_array[0].len()),
                string_array.into_iter().flatten().collect(),
            );
        }

        if numeric_features.is_some() && numeric_array.is_some() {
            let numeric_features = numeric_features.unwrap();
            let num_profile = match self.num_profiler.compute_stats(
                &numeric_features,
                &numeric_array.unwrap().as_array(),
                &bin_size.unwrap_or(20),
            ) {
                Ok(profile) => profile,
                Err(_e) => {
                    return Err(PyValueError::new_err(
                        "Failed to create feature data profile",
                    ));
                }
            };
            profiles.push(num_profile);
        }

        Ok(profiles[0].clone())
    }

    pub fn create_data_profile_f64(
        &mut self,
        features: Vec<String>,
        array: PyReadonlyArray2<f64>,
        bin_size: usize,
    ) -> PyResult<DataProfile> {
        let array = array.as_array();

        let profile = match self
            .num_profiler
            .compute_stats(&features, &array, &bin_size)
        {
            Ok(profile) => profile,
            Err(_e) => {
                return Err(PyValueError::new_err(
                    "Failed to create feature data profile",
                ));
            }
        };

        Ok(profile)
    }
}

#[pyclass]
pub struct ScouterDrifter {
    monitor: Monitor,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl ScouterDrifter {
    #[new]
    pub fn new() -> Self {
        Self {
            monitor: Monitor::new(),
        }
    }

    pub fn create_drift_profile_f32(
        &mut self,
        features: Vec<String>,
        array: PyReadonlyArray2<f32>,
        monitor_config: DriftConfig,
    ) -> PyResult<DriftProfile> {
        let array = array.as_array();

        let profile = match self
            .monitor
            .create_2d_drift_profile(&features, &array, &monitor_config)
        {
            Ok(profile) => profile,
            Err(_e) => {
                return Err(PyValueError::new_err("Failed to create 2D monitor profile"));
            }
        };

        Ok(profile)
    }

    pub fn create_drift_profile_f64(
        &mut self,
        features: Vec<String>,
        array: PyReadonlyArray2<f64>,
        monitor_config: DriftConfig,
    ) -> PyResult<DriftProfile> {
        let array = array.as_array();

        let profile = match self
            .monitor
            .create_2d_drift_profile(&features, &array, &monitor_config)
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
        features: Vec<String>,
        drift_array: PyReadonlyArray2<f32>,
        drift_profile: DriftProfile,
    ) -> PyResult<DriftMap> {
        let array = drift_array.as_array();

        let drift_map = match self
            .monitor
            .compute_drift(&features, &array, &drift_profile)
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
        features: Vec<String>,
        drift_array: PyReadonlyArray2<f64>,
        drift_profile: DriftProfile,
    ) -> PyResult<DriftMap> {
        let array = drift_array.as_array();

        let drift_map = match self
            .monitor
            .compute_drift(&features, &array, &drift_profile)
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
        alert_rule: AlertRule,
    ) -> PyResult<FeatureAlerts> {
        let array = drift_array.as_array();

        let alerts = match generate_alerts(&array, features, alert_rule) {
            Ok(alerts) => alerts,
            Err(_e) => {
                return Err(PyValueError::new_err("Failed to generate alerts"));
            }
        };

        Ok(alerts)
    }

    pub fn sample_data_f32(
        &mut self,
        features: Vec<String>,
        array: PyReadonlyArray2<f32>,
        drift_profile: DriftProfile,
    ) -> PyResult<Vec<DriftServerRecord>> {
        let array = array.as_array();

        let profile = match self.monitor.sample_data(&features, &array, &drift_profile) {
            Ok(profile) => profile,
            Err(_e) => {
                return Err(PyValueError::new_err("Failed to sample data"));
            }
        };

        Ok(profile)
    }

    pub fn sample_data_f64(
        &mut self,
        features: Vec<String>,
        array: PyReadonlyArray2<f64>,
        drift_profile: DriftProfile,
    ) -> PyResult<Vec<DriftServerRecord>> {
        let array = array.as_array();

        let profile = match self.monitor.sample_data(&features, &array, &drift_profile) {
            Ok(profile) => profile,
            Err(_e) => {
                return Err(PyValueError::new_err("Failed to sample data"));
            }
        };

        Ok(profile)
    }
}
