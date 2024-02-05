use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Python class for a feature bin
///
/// # Arguments
///
/// * `bins` - A vector of bins
/// * `bin_counts` - A vector of bin counts
///
#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Bin {
    #[pyo3(get, set)]
    pub bins: Vec<f64>,

    #[pyo3(get, set)]
    pub bin_counts: Vec<i32>,
}

/// Python class for holding distinct data metadata
///
/// # Arguments
///
/// * `count` - The number of distinct values
/// * `percent` - The percentage of distinct values
///
#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Distinct {
    #[pyo3(get, set)]
    pub count: usize,

    #[pyo3(get, set)]
    pub percent: f64,
}

/// Python class for holding infinity data metadata
///
/// # Arguments
///
/// * `count` - The number of infinite values
/// * `percent` - The percentage of infinite values
///
#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Infinity {
    #[pyo3(get, set)]
    pub count: usize,

    #[pyo3(get, set)]
    pub percent: f64,
}

/// Python class for holding missing data metadata
///
/// # Arguments
///
/// * `count` - The number of missing values
/// * `percent` - The percentage of missing values
///
#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Missing {
    #[pyo3(get, set)]
    pub count: usize,

    #[pyo3(get, set)]
    pub percent: f64,
}

/// Python class for holding stats data metadata
///
/// # Arguments
///
/// * `median` - The median value
/// * `mean` - The mean value
/// * `standard_dev` - The standard deviation
/// * `min` - The minimum value
/// * `max` - The maximum value
/// * `distinct` - The distinct data metadata
/// * `infinity` - The infinity data metadata
///
#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Stats {
    #[pyo3(get, set)]
    pub median: f64,

    #[pyo3(get, set)]
    pub mean: f64,

    #[pyo3(get, set)]
    pub standard_dev: f64,

    #[pyo3(get, set)]
    pub min: f64,

    #[pyo3(get, set)]
    pub max: f64,

    #[pyo3(get, set)]
    pub distinct: Distinct,

    #[pyo3(get, set)]
    pub infinity: Infinity,

    #[pyo3(get, set)]
    pub missing: Option<Missing>,
}

/// Python class for holding feature metadata
///
/// # Arguments
///
/// * `name` - The name of the feature
/// * `bins` - The bins of the feature
/// * `stats` - The stats of the feature
///
#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeatureStat {
    #[pyo3(get, set)]
    pub name: String,

    #[pyo3(get, set)]
    pub bins: Bin,

    #[pyo3(get, set)]
    pub stats: Stats,
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeatureStats {
    pub feature: HashMap<String, FeatureStat>,
}

#[pymethods]
impl FeatureStat {
    #[new]
    fn new(name: String, bins: Bin, stats: Stats) -> Self {
        Self {
            name: name,
            bins: bins,
            stats: stats,
        }
    }
}
