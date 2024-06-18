use core::f32;

use scouter::math::monitor::Monitor;
use scouter::math::profiler::Profiler;
use scouter::types::_types::{DataProfile, DriftMap, MonitorConfig, MonitorProfile};

use numpy::PyReadonlyArray2;
use pyo3::exceptions::PyValueError;

use pyo3::prelude::*;

#[pyclass]
pub struct RustScouter {
    monitor: Monitor,
    profiler: Profiler,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl RustScouter {
    #[new]
    pub fn new(bin_size: Option<usize>, monitor_config: Option<MonitorConfig>) -> Self {
        let profiler = match bin_size {
            Some(size) => Profiler::new(size),
            None => Profiler::default(),
        };

        Self {
            monitor: Monitor::new(),
            profiler,
        }
    }

    pub fn create_data_profile_f32(
        &mut self,
        array: PyReadonlyArray2<f32>,
        features: Vec<String>,
    ) -> PyResult<DataProfile> {
        let array = array.as_array();

        let profile = match self.profiler.compute_stats(&features, &array) {
            Ok(profile) => profile,
            Err(_e) => {
                return Err(PyValueError::new_err(
                    "Failed to create feature data profile",
                ));
            }
        };

        Ok(profile)
    }

    pub fn create_data_profile_f64(
        &mut self,
        array: PyReadonlyArray2<f64>,
        features: Vec<String>,
    ) -> PyResult<DataProfile> {
        let array = array.as_array();

        let profile = match self.profiler.compute_stats(&features, &array) {
            Ok(profile) => profile,
            Err(_e) => {
                return Err(PyValueError::new_err(
                    "Failed to create feature data profile",
                ));
            }
        };

        Ok(profile)
    }

    pub fn create_monitor_profile_f32(
        &mut self,
        array: PyReadonlyArray2<f32>,
        features: Vec<String>,
        monitor_config: MonitorConfig,
    ) -> PyResult<MonitorProfile> {
        let array = array.as_array();

        let profile =
            match self
                .monitor
                .create_2d_monitor_profile(&features, &array, &monitor_config)
            {
                Ok(profile) => profile,
                Err(_e) => {
                    return Err(PyValueError::new_err("Failed to create 2D monitor profile"));
                }
            };

        Ok(profile)
    }

    pub fn create_monitor_profile_f64(
        &mut self,
        array: PyReadonlyArray2<f64>,
        features: Vec<String>,
        monitor_config: MonitorConfig,
    ) -> PyResult<MonitorProfile> {
        let array = array.as_array();

        let profile =
            match self
                .monitor
                .create_2d_monitor_profile(&features, &array, &monitor_config)
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
        drift_array: PyReadonlyArray2<f32>,
        features: Vec<String>,
        monitor_profile: MonitorProfile,
    ) -> PyResult<DriftMap> {
        let array = drift_array.as_array();

        let drift_map = match self
            .monitor
            .compute_drift(&array, &features, &monitor_profile)
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
        drift_array: PyReadonlyArray2<f64>,
        features: Vec<String>,
        monitor_profile: MonitorProfile,
    ) -> PyResult<DriftMap> {
        let array = drift_array.as_array();

        let drift_map = match self
            .monitor
            .compute_drift(&array, &features, &monitor_profile)
        {
            Ok(drift_map) => drift_map,
            Err(_e) => {
                return Err(PyValueError::new_err("Failed to compute drift"));
            }
        };

        Ok(drift_map)
    }
}