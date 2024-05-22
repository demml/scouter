use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

enum FileName {
    Drift,
    Profile,
}

impl FileName {
    fn as_str(&self) -> &'static str {
        match self {
            FileName::Drift => "drift_map.json",
            FileName::Profile => "data_profile.json",
        }
    }
}

fn save_to_json<T>(model: T, path: Option<PathBuf>, filename: &str) -> PyResult<()>
where
    T: Serialize,
{
    // serialize the struct to a string
    let json = serde_json::to_string_pretty(&model).unwrap();

    // check if path is provided
    let write_path = if path.is_some() {
        let mut new_path = PathBuf::from(path.unwrap());

        // ensure .json extension
        new_path.set_extension("json");

        // ensure path exists, create if not
        if !new_path.exists() {
            std::fs::create_dir_all(&new_path.parent().unwrap())
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))?;
        }

        new_path
    } else {
        PathBuf::from(filename)
    };

    std::fs::write(write_path, json)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))?;

    Ok(())
}

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
pub struct FeatureDataProfile {
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

#[pymethods]
impl FeatureDataProfile {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        serde_json::to_string_pretty(&self).unwrap()
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DataProfile {
    #[pyo3(get, set)]
    pub features: HashMap<String, FeatureDataProfile>,
}

#[pymethods]
impl DataProfile {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        serde_json::to_string_pretty(&self).unwrap()
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        self.__str__()
    }

    #[staticmethod]
    pub fn load_from_json(model: String) -> DataProfile {
        // deserialize the string to a struct
        let profile: DataProfile = serde_json::from_str(&model).unwrap();
        profile
    }

    pub fn save_to_json(&self, path: Option<PathBuf>) -> PyResult<()> {
        save_to_json(self, path, FileName::Profile.as_str())
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

/// Python class for a feature drift
///
/// # Arguments
///
/// * `samples` - A vector of samples
/// * `drift` - A vector of drift values
///
#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeatureDrift {
    #[pyo3(get, set)]
    pub samples: Vec<f64>,

    #[pyo3(get, set)]
    pub drift: Vec<f64>,
}

impl FeatureDrift {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        serde_json::to_string_pretty(&self).unwrap()
    }
}

/// Python class for a Drift map of features with calculated drift
///
/// # Arguments
///
/// * `features` - A hashmap of feature names and their drift
///
#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DriftMap {
    #[pyo3(get, set)]
    pub features: HashMap<String, FeatureDrift>,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl DriftMap {
    #[new]
    pub fn new() -> Self {
        Self {
            features: HashMap::new(),
        }
    }

    pub fn add_feature(&mut self, feature: String, profile: FeatureDrift) {
        self.features.insert(feature, profile);
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        serde_json::to_string_pretty(&self).unwrap()
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        self.__str__()
    }

    #[staticmethod]
    pub fn load_from_json(model: String) -> DriftMap {
        // deserialize the string to a struct
        let map: DriftMap = serde_json::from_str(&model).unwrap();
        map
    }

    pub fn save_to_json(&self, path: Option<PathBuf>) -> PyResult<()> {
        save_to_json(self, path, FileName::Drift.as_str())
    }
}
