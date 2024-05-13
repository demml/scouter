use core::f32;

use crate::math::stats::{compute_array_stats, Monitor};
use crate::types::_types::MonitorProfile;

use ndarray::prelude::*;

use numpy::PyReadonlyArray2;
use pyo3::exceptions::PyValueError;

use pyo3::prelude::*;
use rayon::prelude::*;

#[pyclass]
pub struct RustScouter {
    monitor: Monitor,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl RustScouter {
    #[new]
    pub fn new() -> Self {
        Self {
            monitor: Monitor::new(),
        }
    }

    pub fn create_data_profile_f32(&mut self, array: PyReadonlyArray2<f32>) -> PyResult<()> {
        let array = array.as_array();

        let stat_vec = array
            .axis_iter(Axis(1))
            .into_par_iter()
            .map(|x| compute_array_stats(&x))
            .collect::<Vec<_>>();

        println!("{:?}", stat_vec);

        Ok(())
    }

    pub fn create_data_profile_f64(&mut self, array: PyReadonlyArray2<f64>) -> PyResult<()> {
        let array = array.as_array();

        let stat_vec = array
            .axis_iter(Axis(1))
            .into_par_iter()
            .map(|x| compute_array_stats(&x))
            .collect::<Vec<_>>();

        println!("{:?}", stat_vec);

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
