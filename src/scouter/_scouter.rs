use crate::math::stats::{check_features, compute_array_stats, create_2d_monitor_profile};
use crate::types::_types::MonitorProfile;

use ndarray::prelude::*;
use numpy::PyReadonlyArray2;
use pyo3::exceptions::PyValueError;
use pyo3::types::PyDict;

use pyo3::prelude::*;
use rayon::prelude::*;

#[pyclass]
pub struct RustScouter {
    features: Vec<String>,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl RustScouter {
    #[new]
    pub fn new(features: Option<Vec<String>>) -> Self {
        Self {
            features: features.unwrap_or_default(),
        }
    }

    pub fn create_data_profile_f32(&mut self, array: PyReadonlyArray2<f32>) -> PyResult<()> {
        let array = array.as_array();

        self.features = match check_features(&self.features, array) {
            Ok(features) => features,
            Err(_e) => {
                return Err(PyValueError::new_err("Failed to check features"));
            }
        };

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

        self.features = match check_features(&self.features, array) {
            Ok(features) => features,
            Err(_e) => {
                return Err(PyValueError::new_err("Failed to check features"));
            }
        };

        let stat_vec = array
            .axis_iter(Axis(1))
            .into_par_iter()
            .map(|x| compute_array_stats(&x))
            .collect::<Vec<_>>();

        println!("{:?}", stat_vec);

        Ok(())
    }

    pub fn create_monitor_profile_f32(&mut self, array: PyReadonlyArray2<f32>) -> PyResult<String> {
        let array = array.as_array();

        let profile = match create_2d_monitor_profile(&self.features, array) {
            Ok(profile) => profile,
            Err(_e) => {
                return Err(PyValueError::new_err("Failed to create 2D monitor profile"));
            }
        };

        let msg = serde_json::to_string(&profile).unwrap();

        Ok(msg)
    }

    pub fn create_monitor_profile_f64(&mut self, array: PyReadonlyArray2<f64>) -> PyResult<String> {
        let array = array.as_array();

        let profile = match create_2d_monitor_profile(&self.features, array) {
            Ok(profile) => profile,
            Err(_e) => {
                return Err(PyValueError::new_err("Failed to create 2D monitor profile"));
            }
        };

        let msg = serde_json::to_string(&profile).unwrap();

        Ok(msg)
    }
}
