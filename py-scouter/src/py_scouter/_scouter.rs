use core::f32;

use scouter::core::monitor::Monitor;
use scouter::core::profiler::Profiler;
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
    pub fn new() -> Self {
        Self {
            monitor: Monitor::new(),
            profiler: Profiler::default(),
        }
    }

    pub fn create_data_profile_f32(
        &mut self,
        features: Vec<String>,
        array: PyReadonlyArray2<f32>,
        bin_size: usize,
    ) -> PyResult<DataProfile> {
        let array = array.as_array();

        let profile = match self.profiler.compute_stats(&features, &array, &bin_size) {
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
        features: Vec<String>,
        array: PyReadonlyArray2<f64>,
        bin_size: usize,
    ) -> PyResult<DataProfile> {
        let array = array.as_array();

        let profile = match self.profiler.compute_stats(&features, &array, &bin_size) {
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
        features: Vec<String>,
        array: PyReadonlyArray2<f32>,
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
        features: Vec<String>,
        array: PyReadonlyArray2<f64>,
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
        features: Vec<String>,
        drift_array: PyReadonlyArray2<f32>,
        monitor_profile: MonitorProfile,
    ) -> PyResult<DriftMap> {
        let array = drift_array.as_array();

        let drift_map = match self
            .monitor
            .compute_drift(&features, &array, &monitor_profile)
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
        monitor_profile: MonitorProfile,
    ) -> PyResult<DriftMap> {
        let array = drift_array.as_array();

        let drift_map = match self
            .monitor
            .compute_drift(&features, &array, &monitor_profile)
        {
            Ok(drift_map) => drift_map,
            Err(_e) => {
                return Err(PyValueError::new_err("Failed to compute drift"));
            }
        };

        Ok(drift_map)
    }
}
