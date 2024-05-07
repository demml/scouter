use crate::logging::logger::Logger;
use crate::math::stats::{compute_array_stats, create_monitor_profile};

use ndarray::prelude::*;
use numpy::PyReadonlyArray2;
use pyo3::prelude::*;
use rayon::prelude::*;

#[pyclass]
pub struct DataProfiler {
    logger: Logger,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl DataProfiler {
    #[new]
    pub fn new() -> Self {
        Self {
            logger: Logger::new(),
        }
    }

    pub fn create_data_profile(&self, array: PyReadonlyArray2<f64>) -> PyResult<()> {
        let _ = &self.logger.info("Creating data profile");

        let array = array.as_array();
        let stat_vec = array
            .axis_iter(Axis(1))
            .into_par_iter()
            .map(|x| compute_array_stats(&x))
            .collect::<Vec<_>>();

        println!("{:?}", stat_vec);

        Ok(())
    }

    pub fn create_monitor_profile(&self, array: PyReadonlyArray2<f64>) -> PyResult<()> {
        &self.logger.info("Creating monitor profile");
        let array = array.as_array();
        let _monitor_vec = array
            .axis_iter(Axis(1))
            .into_par_iter()
            .map(|x| create_monitor_profile(&x, 10))
            .collect::<Vec<_>>();

        Ok(())
    }
}
