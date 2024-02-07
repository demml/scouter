use crate::math::histogram::{compute_bin_counts, compute_bins};
use crate::math::stats::{
    compute_array_stats, compute_max, compute_mean, compute_min, compute_stddev,
};
use crate::types::types::{Bin, Distinct, FeatureStat, Infinity, Stats};
use anyhow::{Context, Result};
use ndarray::prelude::*;
use ndarray_stats::{interpolate::Nearest, QuantileExt};
use noisy_float::prelude::*;
use noisy_float::types::n64;
use num_traits::Float;
use numpy::ndarray::{aview1, ArrayView1, ArrayView2};
use numpy::PyReadonlyArray2;
use pyo3::prelude::*;
use rayon::prelude::*;
use std::collections::HashSet;
use tracing::{debug, error, info, span, warn, Level};

#[pyclass]
pub struct DataProfiler {}

#[pymethods]
impl DataProfiler {
    #[new]
    pub fn new() -> Self {
        DataProfiler {}
    }

    pub fn create_data_profile(&self, array: PyReadonlyArray2<f64>) -> PyResult<()> {
        let array = array.as_array();
        let stat_vec = array
            .axis_iter(Axis(1))
            .into_par_iter()
            .map(|x| compute_array_stats(&x))
            .collect::<Vec<_>>();

        Ok(())
    }
}
