use std::collections::HashMap;

use crate::logging::logger::Logger;
use crate::math::stats::{check_features, compute_array_stats, create_monitor_profile};
use crate::types::_types::MonitorProfile;

use ndarray::prelude::*;
use numpy::PyReadonlyArray2;
use pyo3::exceptions::PyValueError;

use pyo3::prelude::*;
use rayon::prelude::*;

#[pyclass]
pub struct Scouter {
    logger: Logger,
    features: Vec<String>,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl Scouter {
    #[new]
    pub fn new(features: Option<Vec<String>>) -> Self {
        Self {
            logger: Logger::new(),
            features: features.unwrap_or(Vec::new()),
        }
    }

    pub fn create_data_profile(&mut self, array: PyReadonlyArray2<f64>) -> PyResult<()> {
        self.logger.info("Creating data profile");

        let array = array.as_array();

        self.features = match check_features(&self.features, array) {
            Ok(features) => features,
            Err(e) => {
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

    pub fn create_monitor_profile(
        &mut self,
        array: PyReadonlyArray2<f64>,
    ) -> PyResult<HashMap<String, MonitorProfile>> {
        self.logger.info("Creating monitor profile");

        let array = array.as_array();

        self.features = match check_features(&self.features, array) {
            Ok(features) => features,
            Err(e) => {
                return Err(PyValueError::new_err("Failed to check features"));
            }
        };

        let monitor_vec = array
            .axis_iter(Axis(1))
            .into_par_iter()
            .map(|x| create_monitor_profile(&x, 10))
            .collect::<Vec<_>>();

        let mut monitor_profile = HashMap::new();

        // iterate over the monitor_vec and add the monitor profile to the hashmap
        for (i, monitor) in monitor_vec.iter().enumerate() {
            match monitor {
                Ok(monitor) => {
                    monitor_profile.insert(self.features[i].clone(), monitor.clone());
                }
                Err(e) => {
                    self.logger
                        .error(&format!("Failed to create monitor profile: {}", e));
                }
            }
        }

        Ok(monitor_profile)
    }
}
