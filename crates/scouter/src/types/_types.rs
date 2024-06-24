use anyhow::Context;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;

enum FileName {
    Drift,
    Profile,
}

impl FileName {
    fn to_str(&self) -> &'static str {
        match self {
            FileName::Drift => "drift_map.json",
            FileName::Profile => "data_profile.json",
        }
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum AlertRules {
    Standard,
}

#[pymethods]
impl AlertRules {
    pub fn to_str(&self) -> String {
        match self {
            AlertRules::Standard => "8 16 4 8 2 4 1 1".to_string(),
        }
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, std::cmp::Eq, Hash)]
pub enum AlertZone {
    Zone1,
    Zone2,
    Zone3,
    OutOfBounds,
    NotApplicable,
}

#[pymethods]
impl AlertZone {
    pub fn to_str(&self) -> String {
        match self {
            AlertZone::Zone1 => "Zone 1".to_string(),
            AlertZone::Zone2 => "Zone 2".to_string(),
            AlertZone::Zone3 => "Zone 3".to_string(),
            AlertZone::OutOfBounds => "Out of bounds".to_string(),
            AlertZone::NotApplicable => "NA".to_string(),
        }
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Copy)]
pub enum AlertType {
    OutOfBounds,
    Consecutive,
    Alternating,
    AllGood,
    Trend,
}

#[pymethods]
impl AlertType {
    pub fn to_str(&self) -> String {
        match self {
            AlertType::OutOfBounds => "Out of bounds".to_string(),
            AlertType::Consecutive => "Consecutive".to_string(),
            AlertType::Alternating => "Alternating".to_string(),
            AlertType::AllGood => "All good".to_string(),
            AlertType::Trend => "Trend".to_string(),
        }
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, Hash, PartialEq)]
pub struct Alert {
    #[pyo3(get, set)]
    pub kind: String,

    #[pyo3(get, set)]
    pub zone: String,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl Alert {
    #[new]
    pub fn new(kind: String, zone: String) -> Self {
        Self { kind, zone }
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(&self)
    }
}

struct ProfileFuncs {}

impl ProfileFuncs {
    fn __str__<T: Serialize>(object: T) -> String {
        // serialize the struct to a string
        serde_json::to_string_pretty(&object).unwrap()
    }

    fn save_to_json<T>(model: T, path: Option<PathBuf>, filename: &str) -> Result<(), anyhow::Error>
    where
        T: Serialize,
    {
        // serialize the struct to a string
        let json = serde_json::to_string_pretty(&model).unwrap();

        // check if path is provided
        let write_path = if path.is_some() {
            let mut new_path = path.with_context(|| "Failed to get path")?;

            // ensure .json extension
            new_path.set_extension("json");

            if !new_path.exists() {
                // ensure path exists, create if not
                let parent_path = new_path
                    .parent()
                    .with_context(|| "Failed to get parent path")?;

                std::fs::create_dir_all(parent_path)
                    .with_context(|| "Failed to create directory")?;
            }

            new_path
        } else {
            PathBuf::from(filename)
        };

        std::fs::write(write_path, json)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))?;

        Ok(())
    }
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
pub struct FeatureDriftProfile {
    #[pyo3(get, set)]
    pub id: String,

    #[pyo3(get, set)]
    pub center: f64,

    #[pyo3(get, set)]
    pub one_ucl: f64,

    #[pyo3(get, set)]
    pub one_lcl: f64,

    #[pyo3(get, set)]
    pub two_ucl: f64,

    #[pyo3(get, set)]
    pub two_lcl: f64,

    #[pyo3(get, set)]
    pub three_ucl: f64,

    #[pyo3(get, set)]
    pub three_lcl: f64,

    #[pyo3(get, set)]
    pub timestamp: chrono::NaiveDateTime,
}

/// Python class for a monitoring configuration
///
/// # Arguments
///
/// * `sample_size` - The sample size
/// * `sample` - Whether to sample data or not, Default is true
/// * `name` - The name of the model
/// * `repository` - The repository associated with the model
/// * `alerting_rule` - The alerting rule to use for monitoring
///
#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MonitorConfig {
    #[pyo3(get, set)]
    pub sample_size: usize,

    #[pyo3(get, set)]
    pub sample: bool,

    #[pyo3(get, set)]
    pub name: String,

    #[pyo3(get, set)]
    pub repository: String,

    #[pyo3(get, set)]
    pub version: String,

    #[pyo3(get, set)]
    pub alert_rule: String,
}

#[pymethods]
impl MonitorConfig {
    #[new]
    pub fn new(
        name: String,
        repository: String,
        version: Option<String>,
        sample: Option<bool>,
        sample_size: Option<usize>,
        alert_rule: Option<String>,
    ) -> Self {
        let sample = match sample {
            Some(s) => s,
            None => true,
        };

        let sample_size = match sample_size {
            Some(size) => size,
            None => 25,
        };

        let version = match version {
            Some(v) => v,
            None => "0.1.0".to_string(),
        };

        let alert_rule = match alert_rule {
            Some(rule) => rule,
            None => AlertRules::Standard.to_str(),
        };

        Self {
            sample_size,
            sample,
            name,
            repository,
            version,
            alert_rule,
        }
    }

    pub fn set_config(
        &mut self,
        sample: Option<bool>,
        sample_size: Option<usize>,
        name: Option<String>,
        repository: Option<String>,
        version: Option<String>,
        alert_rule: Option<String>,
    ) {
        if sample.is_some() {
            self.sample = sample.unwrap();
        }

        if sample_size.is_some() {
            self.sample_size = sample_size.unwrap();
        }

        if name.is_some() {
            self.name = name.unwrap();
        }

        if repository.is_some() {
            self.repository = repository.unwrap();
        }

        if alert_rule.is_some() {
            self.alert_rule = alert_rule.unwrap();
        }

        if version.is_some() {
            self.version = version.unwrap();
        }
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DriftProfile {
    #[pyo3(get, set)]
    pub features: HashMap<String, FeatureDriftProfile>,

    #[pyo3(get, set)]
    pub config: MonitorConfig,
}

#[pymethods]
impl DriftProfile {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(&self)
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        self.__str__()
    }

    #[staticmethod]
    pub fn load_from_json(model: String) -> DriftProfile {
        // deserialize the string to a struct
        serde_json::from_str(&model).expect("Failed to load monitor profile")
    }

    pub fn save_to_json(&self, path: Option<PathBuf>) -> PyResult<()> {
        ProfileFuncs::save_to_json(self, path, FileName::Profile.to_str())
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))
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
    pub timestamp: chrono::NaiveDateTime,

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
    pub features: HashMap<String, FeatureDataProfile>,
}

#[pymethods]
impl DataProfile {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(&self)
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        self.__str__()
    }

    #[staticmethod]
    pub fn load_from_json(model: String) -> DataProfile {
        // deserialize the string to a struct
        serde_json::from_str(&model).expect("Failed to load data profile")
    }

    pub fn save_to_json(&self, path: Option<PathBuf>) -> PyResult<()> {
        ProfileFuncs::save_to_json(self, path, FileName::Profile.to_str())
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))
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

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DriftServerRecord {
    #[pyo3(get, set)]
    pub created_at: chrono::NaiveDateTime,

    #[pyo3(get, set)]
    pub name: String,

    #[pyo3(get, set)]
    pub repository: String,

    #[pyo3(get, set)]
    pub version: String,

    #[pyo3(get, set)]
    pub feature: String,

    #[pyo3(get, set)]
    pub value: f64,
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

    #[pyo3(get, set)]
    pub name: String,

    #[pyo3(get, set)]
    pub repository: String,

    #[pyo3(get, set)]
    pub version: String,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl DriftMap {
    #[new]
    pub fn new(name: String, repository: String, version: String) -> Self {
        Self {
            features: HashMap::new(),
            name,
            repository,
            version,
        }
    }

    pub fn add_feature(&mut self, feature: String, profile: FeatureDrift) {
        self.features.insert(feature, profile);
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(&self)
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        self.__str__()
    }

    #[staticmethod]
    pub fn load_from_json(model: String) -> DriftMap {
        // deserialize the string to a struct
        serde_json::from_str(&model).expect("Failed to load drift map")
    }

    pub fn save_to_json(&self, path: Option<PathBuf>) -> PyResult<()> {
        ProfileFuncs::save_to_json(self, path, FileName::Drift.to_str())
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))
    }

    // convert drift map to server record for sending to scouter server
    pub fn to_server_record(&self) -> Vec<DriftServerRecord> {
        let mut records = Vec::new();

        for (feature, drift) in &self.features {
            drift.drift.iter().enumerate().for_each(|(i, _)| {
                records.push(DriftServerRecord {
                    created_at: chrono::Utc::now().naive_utc(),
                    name: self.name.clone(),
                    repository: self.repository.clone(),
                    version: self.version.clone(),
                    feature: feature.clone(),
                    value: drift.drift[i],
                });
            });
        }

        records
    }
}

// Drift config to use when calculating drift on a new sample of data
#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DriftConfig {
    #[pyo3(get, set)]
    pub features: Vec<String>,
    pub drift_profile: DriftProfile,
    pub service_name: Option<String>,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl DriftConfig {
    #[new]
    pub fn new(
        features: Vec<String>,
        drift_profile: DriftProfile,
        service_name: Option<String>,
    ) -> Self {
        Self {
            features,
            drift_profile,
            service_name,
        }
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        serde_json::to_string_pretty(&self).unwrap()
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeatureAlert {
    #[pyo3(get, set)]
    pub feature: String,

    #[pyo3(get, set)]
    pub alerts: Vec<Alert>,

    #[pyo3(get, set)]
    pub indices: HashMap<usize, Vec<Vec<usize>>>,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl FeatureAlert {
    #[new]
    pub fn new(feature: String) -> Self {
        Self {
            feature: feature,
            alerts: Vec::new(),
            indices: HashMap::new(),
        }
    }
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(&self)
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeatureAlerts {
    #[pyo3(get, set)]
    pub alerts: HashMap<String, FeatureAlert>,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl FeatureAlerts {
    #[new]
    pub fn new() -> Self {
        Self {
            alerts: HashMap::new(),
        }
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(&self)
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        self.__str__()
    }

    pub fn insert_alert(
        &mut self,
        feature: String,
        alert: Alert,
        indices: &HashMap<usize, Vec<Vec<usize>>>,
    ) {
        let feature_alert = self
            .alerts
            .entry(feature)
            .or_insert(FeatureAlert::new(feature));
        feature_alert.alerts.push(alert);
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_types() {
        // write tests for all alerts
        assert_eq!(AlertRules::Standard.to_str(), "8 16 4 8 2 4 1 1");
        assert_eq!(AlertZone::NotApplicable.to_str(), "NA");
        assert_eq!(AlertZone::Zone1.to_str(), "Zone 1");
        assert_eq!(AlertZone::Zone2.to_str(), "Zone 2");
        assert_eq!(AlertZone::Zone3.to_str(), "Zone 3");
        assert_eq!(AlertZone::OutOfBounds.to_str(), "Out of bounds");
        assert_eq!(AlertType::AllGood.to_str(), "All good");
        assert_eq!(AlertType::Consecutive.to_str(), "Consecutive");
        assert_eq!(AlertType::Alternating.to_str(), "Alternating");
        assert_eq!(AlertType::OutOfBounds.to_str(), "Out of bounds");
    }
}
