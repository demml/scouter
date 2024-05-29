use core::f32;
use std::collections::HashMap;

use crate::math::monitor::Monitor;
use crate::math::profiler::Profiler;
use crate::types::_types::{DataProfile, DriftMap, MonitorProfile};

use numpy::PyReadonlyArray2;
use pyo3::exceptions::PyValueError;

use ndarray::Dim;
use numpy::PyReadonlyArray;
use pyo3::prelude::*;
use pyo3::types::PyList;
use rayon::iter::IntoParallelIterator;

#[pyclass]
pub struct RustScouter {
    monitor: Monitor,
    profiler: Profiler,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl RustScouter {
    #[new]
    pub fn new(bin_size: Option<usize>) -> Self {
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
    ) -> PyResult<MonitorProfile> {
        let array = array.as_array();

        let profile = match self.monitor.create_2d_monitor_profile(&features, &array) {
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

        let profile = match self.monitor.create_2d_monitor_profile(&features, &array) {
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
        monitor_profile: MonitorProfile,
        sample: bool,
        sample_size: Option<usize>,
    ) -> PyResult<DriftMap> {
        let array = array.as_array();

        let drift_map = match self.monitor.compute_drift(
            &features,
            &array,
            &monitor_profile,
            &sample,
            sample_size,
        ) {
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
        monitor_profile: MonitorProfile,
        sample: bool,
        sample_size: Option<usize>,
    ) -> PyResult<DriftMap> {
        let array = array.as_array();

        let drift_map = match self.monitor.compute_drift(
            &features,
            &array,
            &monitor_profile,
            &sample,
            sample_size,
        ) {
            Ok(drift_map) => drift_map,
            Err(_e) => {
                return Err(PyValueError::new_err("Failed to compute drift"));
            }
        };

        Ok(drift_map)
    }
    pub fn compute_many_driftf64(
        &self,
        data: Vec<(Vec<String>, PyReadonlyArray2<f64>, MonitorProfile, String)>,
        sample: bool,
        sample_size: Option<usize>,
    ) -> PyResult<()> {
        // convert to array
        let new_data = data
            .iter()
            .map(|ele| {
                let features = ele.0.clone();
                let array = ele.1.as_array();
                let monitor_profile = ele.2.clone();
                let sample = ele.3.clone();
                (features, array, monitor_profile, sample)
            })
            .collect::<Vec<_>>();

        self.monitor
            .compute_many_drift(new_data, &sample, sample_size);
        Ok(())
    }
}
