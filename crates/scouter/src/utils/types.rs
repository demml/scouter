use crate::utils::cron::EveryDay;
use anyhow::Context;
use colored_json::{Color, ColorMode, ColoredFormatter, PrettyFormatter, Styler};
use core::fmt::Debug;
use ndarray::Array;
use ndarray::Array2;
use numpy::{IntoPyArray, PyArray2};
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::str::FromStr;
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
pub struct ProcessAlertRule {
    #[pyo3(get, set)]
    pub rule: String,

    #[pyo3(get, set)]
    pub zones_to_monitor: Vec<String>,
}

#[pymethods]
impl ProcessAlertRule {
    #[new]
    pub fn new(rule: Option<String>, zones_to_monitor: Option<Vec<String>>) -> Self {
        let rule = match rule {
            Some(r) => r,
            None => "8 16 4 8 2 4 1 1".to_string(),
        };

        let zones = zones_to_monitor.unwrap_or(
            [
                AlertZone::Zone1.to_str(),
                AlertZone::Zone2.to_str(),
                AlertZone::Zone3.to_str(),
                AlertZone::Zone4.to_str(),
            ]
            .to_vec(),
        );
        Self {
            rule,
            zones_to_monitor: zones,
        }
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PercentageAlertRule {
    #[pyo3(get, set)]
    pub rule: f64,
}

#[pymethods]
impl PercentageAlertRule {
    #[new]
    pub fn new(rule: Option<f64>) -> Self {
        let rule = rule.unwrap_or(0.1);
        Self { rule }
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct AlertRule {
    #[pyo3(get, set)]
    pub process: Option<ProcessAlertRule>,

    #[pyo3(get, set)]
    pub percentage: Option<PercentageAlertRule>,
}

// impl new method
#[pymethods]
impl AlertRule {
    #[new]
    pub fn new(
        percentage_rule: Option<PercentageAlertRule>,
        process_rule: Option<ProcessAlertRule>,
    ) -> Self {
        // if both are None, return default control rule
        if percentage_rule.is_none() && process_rule.is_none() {
            return Self {
                process: Some(ProcessAlertRule::new(None, None)),
                percentage: None,
            };
        }

        Self {
            process: process_rule,
            percentage: percentage_rule,
        }
    }

    pub fn to_str(&self) -> String {
        if self.process.is_some() {
            return self.process.as_ref().unwrap().rule.clone();
        } else {
            return self.percentage.as_ref().unwrap().rule.to_string();
        }
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum AlertDispatchType {
    Email,
    Slack,
    Console,
    OpsGenie,
}

#[pymethods]
impl AlertDispatchType {
    #[getter]
    pub fn value(&self) -> String {
        match self {
            AlertDispatchType::Email => "Email".to_string(),
            AlertDispatchType::Slack => "Slack".to_string(),
            AlertDispatchType::Console => "Console".to_string(),
            AlertDispatchType::OpsGenie => "OpsGenie".to_string(),
        }
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct AlertConfig {
    #[pyo3(get, set)]
    pub alert_rule: AlertRule,

    pub alert_dispatch_type: AlertDispatchType,

    #[pyo3(get, set)]
    pub schedule: String,

    #[pyo3(get, set)]
    pub features_to_monitor: Vec<String>,

    #[pyo3(get, set)]
    pub alert_kwargs: HashMap<String, String>,
}

#[pymethods]
impl AlertConfig {
    #[new]
    pub fn new(
        alert_rule: Option<AlertRule>,
        alert_dispatch_type: Option<AlertDispatchType>,
        schedule: Option<String>,
        features_to_monitor: Option<Vec<String>>,
        alert_kwargs: Option<HashMap<String, String>>,
    ) -> Self {
        let alert_rule = alert_rule.unwrap_or(AlertRule::new(None, None));

        let schedule = match schedule {
            Some(s) => {
                // validate the cron schedule
                let schedule = cron::Schedule::from_str(&s);

                match schedule {
                    Ok(_) => s,
                    Err(_) => {
                        tracing::error!("Invalid cron schedule, using default schedule");
                        EveryDay::new().cron
                    }
                }
            }

            None => EveryDay::new().cron,
        };
        let alert_dispatch_type = alert_dispatch_type.unwrap_or(AlertDispatchType::Console);
        let features_to_monitor = features_to_monitor.unwrap_or_default();
        let alert_kwargs = alert_kwargs.unwrap_or_default();

        Self {
            alert_rule,
            alert_dispatch_type,
            schedule,
            features_to_monitor,
            alert_kwargs,
        }
    }

    #[getter]
    pub fn alert_dispatch_type(&self) -> String {
        match self.alert_dispatch_type {
            AlertDispatchType::Email => "Email".to_string(),
            AlertDispatchType::Slack => "Slack".to_string(),
            AlertDispatchType::Console => "Console".to_string(),
            AlertDispatchType::OpsGenie => "OpsGenie".to_string(),
        }
    }
}

impl Default for AlertConfig {
    fn default() -> AlertConfig {
        Self {
            alert_rule: AlertRule::new(None, None),
            alert_dispatch_type: AlertDispatchType::Console,
            schedule: EveryDay::new().cron,
            features_to_monitor: Vec::new(),
            alert_kwargs: HashMap::new(),
        }
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, std::cmp::Eq, Hash)]
pub enum AlertZone {
    Zone1,
    Zone2,
    Zone3,
    Zone4,
    NotApplicable,
}

#[pymethods]
impl AlertZone {
    pub fn to_str(&self) -> String {
        match self {
            AlertZone::Zone1 => "Zone 1".to_string(),
            AlertZone::Zone2 => "Zone 2".to_string(),
            AlertZone::Zone3 => "Zone 3".to_string(),
            AlertZone::Zone4 => "Zone 4".to_string(),
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
    Percentage,
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
            AlertType::Percentage => "Percentage".to_string(),
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
        ProfileFuncs::__str__(self)
    }
}

struct ProfileFuncs {}

impl ProfileFuncs {
    fn __str__<T: Serialize>(object: T) -> String {
        match ColoredFormatter::with_styler(
            PrettyFormatter::default(),
            Styler {
                key: Color::Rgb(245, 77, 85).bold(),
                string_value: Color::Rgb(249, 179, 93).foreground(),
                float_value: Color::Rgb(249, 179, 93).foreground(),
                integer_value: Color::Rgb(249, 179, 93).foreground(),
                bool_value: Color::Rgb(249, 179, 93).foreground(),
                nil_value: Color::Rgb(249, 179, 93).foreground(),
                ..Default::default()
            },
        )
        .to_colored_json(&object, ColorMode::On)
        {
            Ok(json) => json,
            Err(e) => format!("Failed to serialize to json: {}", e),
        }
        // serialize the struct to a string
    }

    fn __json__<T: Serialize>(object: T) -> String {
        match serde_json::to_string_pretty(&object) {
            Ok(json) => json,
            Err(e) => format!("Failed to serialize to json: {}", e),
        }
    }

    fn save_to_json<T>(model: T, path: Option<PathBuf>, filename: &str) -> Result<(), anyhow::Error>
    where
        T: Serialize,
    {
        // serialize the struct to a string
        let json = serde_json::to_string_pretty(&model).with_context(|| "Failed to serialize")?;

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

        std::fs::write(write_path, json).with_context(|| "Failed to write to file")?;

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
/// * `version` - The version of the model
/// * `schedule` - The cron schedule for monitoring
/// * `alert_rule` - The alerting rule to use for monitoring
///
#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DriftConfig {
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
    pub alert_config: AlertConfig,

    #[pyo3(get, set)]
    pub feature_map: Option<FeatureMap>,

    #[pyo3(get, set)]
    pub targets: Vec<String>,
}

#[pymethods]
#[allow(clippy::too_many_arguments)]
impl DriftConfig {
    #[new]
    pub fn new(
        name: Option<String>,
        repository: Option<String>,
        version: Option<String>,
        sample: Option<bool>,
        sample_size: Option<usize>,
        feature_map: Option<FeatureMap>,
        targets: Option<Vec<String>>,
        alert_config: Option<AlertConfig>,
        config_path: Option<PathBuf>,
    ) -> Result<Self, anyhow::Error> {
        if let Some(config_path) = config_path {
            let config = DriftConfig::load_from_json(config_path);
            return config;
        }

        let name = name.unwrap_or("_NA".to_string());
        let repository = repository.unwrap_or("_NA".to_string());
        let sample = sample.unwrap_or(true);
        let sample_size = sample_size.unwrap_or(25);
        let version = version.unwrap_or("0.1.0".to_string());
        let targets = targets.unwrap_or_default();

        let alert_config = alert_config.unwrap_or(AlertConfig::new(None, None, None, None, None));

        Ok(Self {
            sample_size,
            sample,
            name,
            repository,
            version,
            alert_config,
            feature_map,
            targets,
        })
    }

    pub fn update_feature_map(&mut self, feature_map: FeatureMap) {
        self.feature_map = Some(feature_map);
    }

    #[staticmethod]
    pub fn load_from_json(path: PathBuf) -> Result<DriftConfig, anyhow::Error> {
        // deserialize the string to a struct

        let file = std::fs::read_to_string(&path)
            .with_context(|| "Failed to read file")
            .unwrap();

        serde_json::from_str(&file).with_context(|| "Failed to deserialize json")
    }
}

impl DriftConfig {
    pub fn load_map_from_json(path: PathBuf) -> Result<HashMap<String, Value>, anyhow::Error> {
        // deserialize the string to a struct
        let file = std::fs::read_to_string(&path)?;
        let config = serde_json::from_str(&file)?;
        Ok(config)
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DriftProfile {
    #[pyo3(get, set)]
    pub features: BTreeMap<String, FeatureDriftProfile>,

    #[pyo3(get, set)]
    pub config: DriftConfig,

    #[pyo3(get, set)]
    pub scouter_version: String,
}

#[pymethods]
impl DriftProfile {
    #[new]
    pub fn new(features: BTreeMap<String, FeatureDriftProfile>, config: DriftConfig) -> Self {
        let scouter_version = env!("CARGO_PKG_VERSION").to_string();
        Self {
            features,
            config,
            scouter_version,
        }
    }
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__json__(self)
    }

    #[staticmethod]
    pub fn load_from_json(model: String) -> DriftProfile {
        // deserialize the string to a struct
        serde_json::from_str(&model).expect("Failed to load monitor profile")
    }

    pub fn save_to_json(&self, path: Option<PathBuf>) -> Result<(), anyhow::Error> {
        ProfileFuncs::save_to_json(self, path, FileName::Profile.to_str())
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeatureMap {
    #[pyo3(get, set)]
    pub features: BTreeMap<String, BTreeMap<String, usize>>,
}

#[pymethods]
impl FeatureMap {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
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
pub struct NumericStats {
    #[pyo3(get, set)]
    pub mean: f64,

    #[pyo3(get, set)]
    pub stddev: f64,

    #[pyo3(get, set)]
    pub min: f64,

    #[pyo3(get, set)]
    pub max: f64,

    #[pyo3(get, set)]
    pub distinct: Distinct,

    #[pyo3(get, set)]
    pub quantiles: Quantiles,

    #[pyo3(get, set)]
    pub histogram: Histogram,
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CharStats {
    #[pyo3(get, set)]
    pub min_length: usize,

    #[pyo3(get, set)]
    pub max_length: usize,

    #[pyo3(get, set)]
    pub median_length: usize,

    #[pyo3(get, set)]
    pub mean_length: f64,
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WordStats {
    #[pyo3(get, set)]
    pub words: BTreeMap<String, Distinct>,
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StringStats {
    #[pyo3(get, set)]
    pub distinct: Distinct,

    #[pyo3(get, set)]
    pub char_stats: CharStats,

    #[pyo3(get, set)]
    pub word_stats: WordStats,
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeatureProfile {
    #[pyo3(get, set)]
    pub id: String,

    #[pyo3(get, set)]
    pub numeric_stats: Option<NumericStats>,

    #[pyo3(get, set)]
    pub string_stats: Option<StringStats>,

    #[pyo3(get, set)]
    pub timestamp: chrono::NaiveDateTime,
}

#[pymethods]
impl FeatureProfile {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DataProfile {
    #[pyo3(get, set)]
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
    pub fn load_from_json(model: String) -> DataProfile {
        // deserialize the string to a struct
        serde_json::from_str(&model).expect("Failed to load data profile")
    }

    pub fn save_to_json(&self, path: Option<PathBuf>) -> Result<(), anyhow::Error> {
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

#[pymethods]
impl DriftServerRecord {
    #[new]
    pub fn new(
        name: String,
        repository: String,
        version: String,
        feature: String,
        value: f64,
    ) -> Self {
        Self {
            created_at: chrono::Utc::now().naive_utc(),
            name,
            repository,
            version,
            feature,
            value,
        }
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__json__(self)
    }

    pub fn to_dict(&self) -> HashMap<String, String> {
        let mut record = HashMap::new();
        record.insert("created_at".to_string(), self.created_at.to_string());
        record.insert("name".to_string(), self.name.clone());
        record.insert("repository".to_string(), self.repository.clone());
        record.insert("version".to_string(), self.version.clone());
        record.insert("feature".to_string(), self.feature.clone());
        record.insert("value".to_string(), self.value.to_string());
        record
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
    pub features: BTreeMap<String, FeatureDrift>,

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
            features: BTreeMap::new(),
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
        ProfileFuncs::__str__(self)
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__json__(self)
    }

    #[staticmethod]
    pub fn load_from_json(model: String) -> DriftMap {
        // deserialize the string to a struct
        serde_json::from_str(&model).expect("Failed to load drift map")
    }

    pub fn save_to_json(&self, path: Option<PathBuf>) -> Result<(), anyhow::Error> {
        ProfileFuncs::save_to_json(self, path, FileName::Drift.to_str())
    }

    #[allow(clippy::type_complexity)]
    pub fn to_numpy<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<(
        Bound<'py, PyArray2<f64>>,
        Bound<'py, PyArray2<f64>>,
        Vec<String>,
    )> {
        let (drift_array, sample_array, features) = self.to_array().unwrap();

        Ok((
            drift_array.into_pyarray_bound(py).to_owned(),
            sample_array.into_pyarray_bound(py).to_owned(),
            features,
        ))
    }
}

type ArrayReturn = (Array2<f64>, Array2<f64>, Vec<String>);

impl DriftMap {
    pub fn to_array(&self) -> Result<ArrayReturn, anyhow::Error> {
        let columns = self.features.len();
        let rows = self.features.values().next().unwrap().samples.len();

        // create empty array
        let mut drift_array = Array2::<f64>::zeros((rows, columns));
        let mut sample_array = Array2::<f64>::zeros((rows, columns));
        let mut features = Vec::new();

        // iterate over the features and insert the drift values
        for (i, (feature, drift)) in self.features.iter().enumerate() {
            features.push(feature.clone());
            drift_array
                .column_mut(i)
                .assign(&Array::from(drift.drift.clone()));
            sample_array
                .column_mut(i)
                .assign(&Array::from(drift.samples.clone()));
        }

        Ok((drift_array, sample_array, features))
    }
}
// Drift config to use when calculating drift on a new sample of data

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeatureAlert {
    #[pyo3(get, set)]
    pub feature: String,

    #[pyo3(get, set)]
    pub alerts: Vec<Alert>,

    #[pyo3(get, set)]
    pub indices: BTreeMap<usize, Vec<Vec<usize>>>,
}

impl FeatureAlert {
    pub fn new(feature: String) -> Self {
        Self {
            feature,
            alerts: Vec::new(),
            indices: BTreeMap::new(),
        }
    }
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl FeatureAlert {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeatureAlerts {
    #[pyo3(get, set)]
    pub features: BTreeMap<String, FeatureAlert>,

    #[pyo3(get, set)]
    pub has_alerts: bool,
}

impl FeatureAlerts {
    // rust-only function to insert feature alerts
    pub fn insert_feature_alert(
        &mut self,
        feature: &str,
        alerts: &HashSet<Alert>,
        indices: &BTreeMap<usize, Vec<Vec<usize>>>,
    ) {
        let mut feature_alert = FeatureAlert::new(feature.to_string());

        // insert the alerts and indices into the feature alert
        alerts.iter().for_each(|alert| {
            feature_alert.alerts.push(Alert {
                zone: alert.zone.clone(),
                kind: alert.kind.clone(),
            })
        });

        // insert the indices into the feature alert
        indices.iter().for_each(|(key, value)| {
            feature_alert.indices.insert(*key, value.clone());
        });

        self.features.insert(feature.to_string(), feature_alert);
    }
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl FeatureAlerts {
    #[new]
    pub fn new(has_alerts: bool) -> Self {
        Self {
            features: BTreeMap::new(),
            has_alerts,
        }
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__json__(self)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_types() {
        // write tests for all alerts
        let control_alert = AlertRule::new(None, Some(ProcessAlertRule::new(None, None)));

        assert_eq!(control_alert.to_str(), "8 16 4 8 2 4 1 1");
        assert_eq!(AlertZone::NotApplicable.to_str(), "NA");
        assert_eq!(AlertZone::Zone1.to_str(), "Zone 1");
        assert_eq!(AlertZone::Zone2.to_str(), "Zone 2");
        assert_eq!(AlertZone::Zone3.to_str(), "Zone 3");
        assert_eq!(AlertZone::Zone4.to_str(), "Zone 4");
        assert_eq!(AlertType::AllGood.to_str(), "All good");
        assert_eq!(AlertType::Consecutive.to_str(), "Consecutive");
        assert_eq!(AlertType::Alternating.to_str(), "Alternating");
        assert_eq!(AlertType::OutOfBounds.to_str(), "Out of bounds");
        assert_eq!(AlertType::Percentage.to_str(), "Percentage");

        let rule = PercentageAlertRule::new(None);
        assert_eq!(rule.rule, 0.1);
    }

    #[test]
    fn test_alert_config() {
        //test console alert config
        let alert_config = AlertConfig::new(None, None, None, None, None);
        assert_eq!(alert_config.alert_dispatch_type, AlertDispatchType::Console);
        assert_eq!(alert_config.alert_dispatch_type(), "Console");
        assert_eq!(AlertDispatchType::Console.value(), "Console");

        //test email alert config
        let alert_config = AlertConfig::new(None, Some(AlertDispatchType::Email), None, None, None);
        assert_eq!(alert_config.alert_dispatch_type, AlertDispatchType::Email);
        assert_eq!(alert_config.alert_dispatch_type(), "Email");
        assert_eq!(AlertDispatchType::Email.value(), "Email");

        //test slack alert config
        let alert_config = AlertConfig::new(None, Some(AlertDispatchType::Slack), None, None, None);
        assert_eq!(alert_config.alert_dispatch_type, AlertDispatchType::Slack);
        assert_eq!(alert_config.alert_dispatch_type(), "Slack");
        assert_eq!(AlertDispatchType::Slack.value(), "Slack");

        //test opsgenie alert config
        let mut alert_kwargs = HashMap::new();
        alert_kwargs.insert("channel".to_string(), "test".to_string());

        let alert_config = AlertConfig::new(
            None,
            Some(AlertDispatchType::OpsGenie),
            None,
            None,
            Some(alert_kwargs),
        );
        assert_eq!(
            alert_config.alert_dispatch_type,
            AlertDispatchType::OpsGenie
        );
        assert_eq!(alert_config.alert_dispatch_type(), "OpsGenie");
        assert_eq!(alert_config.alert_kwargs.get("channel").unwrap(), "test");
        assert_eq!(AlertDispatchType::OpsGenie.value(), "OpsGenie");
    }
}
