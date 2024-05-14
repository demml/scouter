use core::f32;

use crate::math::monitor::Monitor;
use crate::math::profiler::Profiler;
use crate::types::_types::MonitorProfile;

use ndarray::prelude::*;

use numpy::PyReadonlyArray2;
use pyo3::exceptions::PyValueError;

use pyo3::prelude::*;
use rayon::prelude::*;

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
            profiler: Profiler::new(),
        }
    }

    pub fn create_data_profile_f32(
        &mut self,
        array: PyReadonlyArray2<f32>,
        features: Vec<String>,
    ) -> PyResult<()> {
        let array = array.as_array();

        self.profiler.compute_stats(&array, &features);

        Ok(())
    }

    pub fn create_data_profile_f64(
        &mut self,
        array: PyReadonlyArray2<f64>,
        features: Vec<String>,
    ) -> PyResult<()> {
        let array = array.as_array();

        self.profiler.compute_stats(&array, &features);

        Ok(())
    }

    pub fn create_monitor_profile_f32(
        &mut self,
        array: PyReadonlyArray2<f32>,
        features: Vec<String>,
    ) -> PyResult<MonitorProfile> {
        let array = array.as_array();

        let profile = match self.monitor.create_2d_monitor_profile(&features, array) {
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
    ) -> PyResult<MonitorProfile> {
        let array = array.as_array();

        let profile = match self.monitor.create_2d_monitor_profile(&features, array) {
            Ok(profile) => profile,
            Err(_e) => {
                return Err(PyValueError::new_err("Failed to create 2D monitor profile"));
            }
        };

        Ok(profile)
    }
}
