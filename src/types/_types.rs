use num_traits::Num;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Python class for a monitoring profile
///
/// # Arguments
///
/// * `id` - The id value
/// * `center` - The center value
/// * `ucl` - The upper control limit
/// * `lcl` - The lower control limit
/// * `timestamp` - The timestamp value
///
#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeatureMonitorProfile {
    #[pyo3(get, set)]
    pub id: String,

    #[pyo3(get, set)]
    pub center: f64,

    #[pyo3(get, set)]
    pub ucl: f64,

    #[pyo3(get, set)]
    pub lcl: f64,

    #[pyo3(get, set)]
    pub timestamp: String,
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MonitorProfile {
    #[pyo3(get, set)]
    pub features: HashMap<String, FeatureMonitorProfile>,
}

#[pymethods]
impl MonitorProfile {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        serde_json::to_string_pretty(&self).unwrap()
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Distinct {
    #[pyo3(get, set)]
    pub count: usize,

    #[pyo3(get, set)]
    pub percent: f64,
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeatureProfile {
    #[pyo3(get, set)]
    pub id: String,

    #[pyo3(get, set)]
    pub mean: f64,

    #[pyo3(get, set)]
    pub stddev: f64,

    #[pyo3(get, set)]
    pub min: f64,

    #[pyo3(get, set)]
    pub max: f64,

    #[pyo3(get, set)]
    pub timestamp: String,

    #[pyo3(get, set)]
    pub distinct: Distinct,

    #[pyo3(get, set)]
    pub quantiles: Quantiles,

    #[pyo3(get, set)]
    pub histogram: Histogram,
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DataProfile {
    #[pyo3(get, set)]
    pub features: HashMap<String, FeatureProfile>,
}

#[pymethods]
impl DataProfile {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        serde_json::to_string_pretty(&self).unwrap()
    }
}

/// Python class for quantiles
///
/// # Arguments
///
/// * `quant_25` - The 25th percentile
/// * `quant_50` - The 50th percentile
/// * `quant_75` - The 75th percentile
/// * `quant_99` - The 99th percentile
///
///
#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Quantiles {
    #[pyo3(get, set)]
    pub q25: f64,

    #[pyo3(get, set)]
    pub q50: f64,

    #[pyo3(get, set)]
    pub q75: f64,

    #[pyo3(get, set)]
    pub q99: f64,
}

/// Python class for a feature histogram
///
/// # Arguments
///
/// * `bins` - A vector of bins
/// * `bin_counts` - A vector of bin counts
///
#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Histogram {
    #[pyo3(get, set)]
    pub bins: Vec<f64>,

    #[pyo3(get, set)]
    pub bin_counts: Vec<i32>,
}
