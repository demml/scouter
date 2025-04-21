use chrono::Utc;
use core::fmt::Debug;
use pyo3::prelude::*;
use scouter_error::ScouterError;
use scouter_error::UtilError;
use scouter_types::{FileName, ProfileFuncs};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::PathBuf;

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Distinct {
    #[pyo3(get)]
    pub count: usize,

    #[pyo3(get)]
    pub percent: f64,
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NumericStats {
    #[pyo3(get)]
    pub mean: f64,

    #[pyo3(get)]
    pub stddev: f64,

    #[pyo3(get)]
    pub min: f64,

    #[pyo3(get)]
    pub max: f64,

    #[pyo3(get)]
    pub distinct: Distinct,

    #[pyo3(get)]
    pub quantiles: Quantiles,

    #[pyo3(get)]
    pub histogram: Histogram,
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CharStats {
    #[pyo3(get)]
    pub min_length: usize,

    #[pyo3(get)]
    pub max_length: usize,

    #[pyo3(get)]
    pub median_length: usize,

    #[pyo3(get)]
    pub mean_length: f64,
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WordStats {
    #[pyo3(get)]
    pub words: HashMap<String, Distinct>,
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StringStats {
    #[pyo3(get)]
    pub distinct: Distinct,

    #[pyo3(get)]
    pub char_stats: CharStats,

    #[pyo3(get)]
    pub word_stats: WordStats,
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeatureProfile {
    #[pyo3(get)]
    pub id: String,

    #[pyo3(get)]
    pub numeric_stats: Option<NumericStats>,

    #[pyo3(get)]
    pub string_stats: Option<StringStats>,

    #[pyo3(get)]
    pub timestamp: chrono::DateTime<Utc>,

    #[pyo3(get)]
    pub correlations: Option<HashMap<String, f32>>,
}

#[pymethods]
impl FeatureProfile {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}

impl FeatureProfile {
    pub fn add_correlations(&mut self, correlations: HashMap<String, f32>) {
        self.correlations = Some(correlations);
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DataProfile {
    #[pyo3(get)]
    pub features: BTreeMap<String, FeatureProfile>,
}

#[pymethods]
impl DataProfile {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__json__(self)
    }

    #[staticmethod]
    pub fn model_validate_json(json_string: String) -> DataProfile {
        // deserialize the string to a struct
        serde_json::from_str(&json_string)
            .map_err(UtilError::traced_deserialize_error)
            .unwrap()
    }

    #[pyo3(signature = (path=None))]
    pub fn save_to_json(&self, path: Option<PathBuf>) -> Result<(), ScouterError> {
        ProfileFuncs::save_to_json(self, path, FileName::Profile.to_str())
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
    #[pyo3(get)]
    pub q25: f64,

    #[pyo3(get)]
    pub q50: f64,

    #[pyo3(get)]
    pub q75: f64,

    #[pyo3(get)]
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
    #[pyo3(get)]
    pub bins: Vec<f64>,

    #[pyo3(get)]
    pub bin_counts: Vec<i32>,
}
