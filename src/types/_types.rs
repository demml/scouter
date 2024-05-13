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
    fn __str__(&self) -> String {
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
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Quantiles<F>
where
    F: Num,
{
    pub quant_25: F,

    pub quant_50: F,

    pub quant_75: F,

    pub quant_99: F,
}

/// Python class for a feature bin
///
/// # Arguments
///
/// * `bins` - A vector of bins
/// * `bin_counts` - A vector of bin counts
///

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Bin {
    pub bins: Vec<f64>,

    pub bin_counts: Vec<i32>,
}

/// Python class for holding distinct data metadata
///
/// # Arguments
///
/// * `count` - The number of distinct values
/// * `percent` - The percentage of distinct values
///

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Distinct {
    pub count: usize,

    pub percent: f64,
}

/// Python class for holding infinity data metadata
///
/// # Arguments
///
/// * `count` - The number of infinite values
/// * `percent` - The percentage of infinite values
///

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Infinity {
    pub count: usize,

    pub percent: f64,
}

/// Python class for holding missing data metadata
///
/// # Arguments
///
/// * `count` - The number of missing values
/// * `percent` - The percentage of missing values
///
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Missing {
    pub count: usize,

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
/// * `missing` - The missing data metadata
/// * `quantiles` - The quantiles data metadata
///

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Stats<F: Num> {
    pub mean: f64,

    pub standard_dev: f64,

    pub min: f64,

    pub max: f64,

    pub distinct: Distinct,

    pub infinity: Infinity,

    pub missing: Missing,

    pub quantiles: Quantiles<F>,
}

/// Python class for holding feature metadata
///
/// # Arguments
///
/// * `name` - The name of the feature
/// * `bins` - The bins of the feature
/// * `stats` - The stats of the feature
///

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeatureStat<F: Num> {
    pub name: String,

    pub bins: Bin,

    pub stats: Stats<F>,
}
