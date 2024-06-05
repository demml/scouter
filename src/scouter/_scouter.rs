use core::f32;

use crate::math::monitor::Monitor;
use crate::math::profiler::Profiler;
use crate::types::_types::{DataProfile, DriftConfig, DriftMap, MonitorConfig, MonitorProfile};

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
        monitor_config: Option<MonitorConfig>,
    ) -> PyResult<MonitorProfile> {
        let array = array.as_array();

        let profile =
            match self
                .monitor
                .create_2d_monitor_profile(&features, &array, monitor_config)
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
        monitor_config: Option<MonitorConfig>,
    ) -> PyResult<MonitorProfile> {
        let array = array.as_array();

        let profile =
            match self
                .monitor
                .create_2d_monitor_profile(&features, &array, monitor_config)
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
        drift_sample: (PyReadonlyArray2<f32>, DriftConfig),
    ) -> PyResult<DriftMap> {
        // get arrayview from py,pyarray2<f32>
        let array = drift_sample.0.as_array();
        let config = drift_sample.1;

        let drift_map = match self.monitor.compute_drift(&array, &config) {
            Ok(drift_map) => drift_map,
            Err(_e) => {
                return Err(PyValueError::new_err("Failed to compute drift"));
            }
        };

        Ok(drift_map)
    }

    pub fn compute_drift_f64(
        &mut self,
        drift_sample: (PyReadonlyArray2<f64>, DriftConfig),
    ) -> PyResult<DriftMap> {
        let array = drift_sample.0.as_array();
        let config = drift_sample.1;

        let drift_map = match self.monitor.compute_drift(&array, &config) {
            Ok(drift_map) => drift_map,
            Err(_e) => {
                return Err(PyValueError::new_err("Failed to compute drift"));
            }
        };

        Ok(drift_map)
    }
}

#[pyclass]
pub struct Drifter {
    monitor: Monitor,
    monitor_profile: MonitorProfile,
}
