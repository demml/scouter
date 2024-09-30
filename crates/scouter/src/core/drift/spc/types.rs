use crate::core::cron::EveryDay;
use crate::core::dispatch::types::AlertDispatchType;
use crate::core::drift::base::{DispatchAlertDescription, DispatchDriftConfig, DriftArgs};
use crate::core::error::ScouterError;
use crate::core::utils::{json_to_pyobject, pyobject_to_json, DriftType, FileName, ProfileFuncs};
use core::fmt::Debug;
use ndarray::Array;
use ndarray::Array2;
use numpy::{IntoPyArray, PyArray2};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::str::FromStr;
use tracing::debug;

const MISSING: &str = "__missing__";

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
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct SpcAlertRule {
    #[pyo3(get, set)]
    pub rule: String,

    #[pyo3(get, set)]
    pub zones_to_monitor: Vec<String>,
}

#[pymethods]
impl SpcAlertRule {
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

impl Default for SpcAlertRule {
    fn default() -> SpcAlertRule {
        Self {
            rule: "8 16 4 8 2 4 1 1".to_string(),
            zones_to_monitor: vec![
                AlertZone::Zone1.to_str(),
                AlertZone::Zone2.to_str(),
                AlertZone::Zone3.to_str(),
                AlertZone::Zone4.to_str(),
            ],
        }
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct SpcAlertConfig {
    #[pyo3(get, set)]
    pub rule: SpcAlertRule,

    pub dispatch_type: AlertDispatchType,

    #[pyo3(get, set)]
    pub schedule: String,

    #[pyo3(get, set)]
    pub features_to_monitor: Vec<String>,

    #[pyo3(get, set)]
    pub dispatch_kwargs: HashMap<String, String>,
}

#[pymethods]
impl SpcAlertConfig {
    #[new]
    pub fn new(
        rule: Option<SpcAlertRule>,
        dispatch_type: Option<AlertDispatchType>,
        schedule: Option<String>,
        features_to_monitor: Option<Vec<String>>,
        dispatch_kwargs: Option<HashMap<String, String>>,
    ) -> Self {
        let rule = rule.unwrap_or_default();

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
        let dispatch_type = dispatch_type.unwrap_or_default();
        let features_to_monitor = features_to_monitor.unwrap_or_default();
        let dispatch_kwargs = dispatch_kwargs.unwrap_or_default();

        Self {
            rule,
            dispatch_type,
            schedule,
            features_to_monitor,
            dispatch_kwargs,
        }
    }

    #[getter]
    pub fn dispatch_type(&self) -> String {
        match self.dispatch_type {
            AlertDispatchType::Slack => "Slack".to_string(),
            AlertDispatchType::Console => "Console".to_string(),
            AlertDispatchType::OpsGenie => "OpsGenie".to_string(),
        }
    }
}

impl Default for SpcAlertConfig {
    fn default() -> SpcAlertConfig {
        Self {
            rule: SpcAlertRule::default(),
            dispatch_type: AlertDispatchType::default(),
            schedule: EveryDay::new().cron,
            features_to_monitor: Vec::new(),
            dispatch_kwargs: HashMap::new(),
        }
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Copy)]
pub enum SpcAlertType {
    OutOfBounds,
    Consecutive,
    Alternating,
    AllGood,
    Trend,
}

#[pymethods]
impl SpcAlertType {
    pub fn to_str(&self) -> String {
        match self {
            SpcAlertType::OutOfBounds => "Out of bounds".to_string(),
            SpcAlertType::Consecutive => "Consecutive".to_string(),
            SpcAlertType::Alternating => "Alternating".to_string(),
            SpcAlertType::AllGood => "All good".to_string(),
            SpcAlertType::Trend => "Trend".to_string(),
        }
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, Hash, PartialEq)]
pub struct SpcAlert {
    #[pyo3(get)]
    pub kind: String,

    #[pyo3(get)]
    pub zone: String,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl SpcAlert {
    #[new]
    pub fn new(kind: String, zone: String) -> Self {
        Self { kind, zone }
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
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
pub struct SpcFeatureDriftProfile {
    #[pyo3(get)]
    pub id: String,

    #[pyo3(get)]
    pub center: f64,

    #[pyo3(get)]
    pub one_ucl: f64,

    #[pyo3(get)]
    pub one_lcl: f64,

    #[pyo3(get)]
    pub two_ucl: f64,

    #[pyo3(get)]
    pub two_lcl: f64,

    #[pyo3(get)]
    pub three_ucl: f64,

    #[pyo3(get)]
    pub three_lcl: f64,

    #[pyo3(get)]
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
pub struct SpcDriftConfig {
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
    pub alert_config: SpcAlertConfig,

    #[pyo3(get, set)]
    pub feature_map: Option<FeatureMap>,

    #[pyo3(get, set)]
    pub targets: Vec<String>,

    #[pyo3(get, set)]
    pub drift_type: DriftType,
}

#[pymethods]
#[allow(clippy::too_many_arguments)]
impl SpcDriftConfig {
    #[new]
    pub fn new(
        name: Option<String>,
        repository: Option<String>,
        version: Option<String>,
        sample: Option<bool>,
        sample_size: Option<usize>,
        feature_map: Option<FeatureMap>,
        targets: Option<Vec<String>>,
        alert_config: Option<SpcAlertConfig>,
        config_path: Option<PathBuf>,
    ) -> Result<Self, ScouterError> {
        if let Some(config_path) = config_path {
            let config = SpcDriftConfig::load_from_json_file(config_path);
            return config;
        }

        let name = name.unwrap_or(MISSING.to_string());
        let repository = repository.unwrap_or(MISSING.to_string());

        if name == MISSING || repository == MISSING {
            debug!("Name and repository were not provided. Defaulting to __missing__");
        }

        let sample = sample.unwrap_or(true);
        let sample_size = sample_size.unwrap_or(25);
        let version = version.unwrap_or("0.1.0".to_string());
        let targets = targets.unwrap_or_default();
        let alert_config = alert_config.unwrap_or_default();

        Ok(Self {
            sample_size,
            sample,
            name,
            repository,
            version,
            alert_config,
            feature_map,
            targets,
            drift_type: DriftType::SPC,
        })
    }

    pub fn update_feature_map(&mut self, feature_map: FeatureMap) {
        self.feature_map = Some(feature_map);
    }

    #[staticmethod]
    pub fn load_from_json_file(path: PathBuf) -> Result<SpcDriftConfig, ScouterError> {
        // deserialize the string to a struct

        let file = std::fs::read_to_string(&path).map_err(|_| ScouterError::ReadError)?;

        serde_json::from_str(&file).map_err(|_| ScouterError::DeSerializeError)
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__json__(self)
    }

    // update the arguments of the drift config
    //
    // # Arguments
    //
    // * `name` - The name of the model
    // * `repository` - The repository associated with the model
    // * `version` - The version of the model
    // * `sample` - Whether to sample data or not, Default is true
    // * `sample_size` - The sample size
    // * `feature_map` - The feature map to use
    // * `targets` - The targets to monitor
    // * `alert_config` - The alerting configuration to use
    //
    #[allow(clippy::too_many_arguments)]
    pub fn update_config_args(
        &mut self,
        name: Option<String>,
        repository: Option<String>,
        version: Option<String>,
        sample: Option<bool>,
        sample_size: Option<usize>,
        feature_map: Option<FeatureMap>,
        targets: Option<Vec<String>>,
        alert_config: Option<SpcAlertConfig>,
    ) -> Result<(), ScouterError> {
        if name.is_some() {
            self.name = name.ok_or(ScouterError::TypeError("name".to_string()))?;
        }

        if repository.is_some() {
            self.repository =
                repository.ok_or(ScouterError::TypeError("repository".to_string()))?;
        }

        if version.is_some() {
            self.version = version.ok_or(ScouterError::TypeError("version".to_string()))?;
        }

        if sample.is_some() {
            self.sample = sample.ok_or(ScouterError::TypeError("sample".to_string()))?;
        }

        if sample_size.is_some() {
            self.sample_size =
                sample_size.ok_or(ScouterError::TypeError("sample size".to_string()))?;
        }

        if feature_map.is_some() {
            self.feature_map = feature_map;
        }

        if targets.is_some() {
            self.targets = targets.ok_or(ScouterError::TypeError("targets".to_string()))?;
        }

        if alert_config.is_some() {
            self.alert_config =
                alert_config.ok_or(ScouterError::TypeError("alert_config".to_string()))?;
        }

        Ok(())
    }
}

impl SpcDriftConfig {
    pub fn load_map_from_json(path: PathBuf) -> Result<HashMap<String, Value>, ScouterError> {
        // deserialize the string to a struct
        let file = std::fs::read_to_string(&path).map_err(|_| ScouterError::ReadError)?;
        let config = serde_json::from_str(&file).map_err(|_| ScouterError::DeSerializeError)?;
        Ok(config)
    }
}

impl DispatchDriftConfig for SpcDriftConfig {
    fn get_drift_args(&self) -> DriftArgs {
        DriftArgs {
            name: self.name.clone(),
            repository: self.repository.clone(),
            version: self.version.clone(),
            dispatch_type: self.alert_config.dispatch_type.clone(),
        }
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpcDriftProfile {
    #[pyo3(get, set)]
    pub features: HashMap<String, SpcFeatureDriftProfile>,

    #[pyo3(get, set)]
    pub config: SpcDriftConfig,

    #[pyo3(get, set)]
    pub scouter_version: String,
}

#[pymethods]
impl SpcDriftProfile {
    #[new]
    pub fn new(
        features: HashMap<String, SpcFeatureDriftProfile>,
        config: SpcDriftConfig,
        scouter_version: Option<String>,
    ) -> Self {
        let scouter_version = scouter_version.unwrap_or(env!("CARGO_PKG_VERSION").to_string());
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

    pub fn model_dump(&self, py: Python) -> PyResult<Py<PyDict>> {
        let json_str = serde_json::to_string(&self).map_err(|_| ScouterError::SerializeError)?;

        let json_value: Value =
            serde_json::from_str(&json_str).map_err(|_| ScouterError::DeSerializeError)?;

        // Create a new Python dictionary
        let dict = PyDict::new_bound(py);

        // Convert JSON to Python dict
        json_to_pyobject(py, &json_value, dict.as_gil_ref())?;

        // Return the Python dictionary
        Ok(dict.into())
    }

    #[staticmethod]
    pub fn model_validate(py: Python, data: &Bound<'_, PyDict>) -> SpcDriftProfile {
        let json_value = pyobject_to_json(py, data.as_gil_ref()).unwrap();

        let string = serde_json::to_string(&json_value).unwrap();
        serde_json::from_str(&string).expect("Failed to load drift profile")
    }

    #[staticmethod]
    pub fn model_validate_json(json_string: String) -> SpcDriftProfile {
        // deserialize the string to a struct
        serde_json::from_str(&json_string).expect("Failed to load monitor profile")
    }

    // Convert python dict into a drift profile
    pub fn save_to_json(&self, path: Option<PathBuf>) -> Result<(), ScouterError> {
        ProfileFuncs::save_to_json(self, path, FileName::Profile.to_str())
    }

    // update the arguments of the drift config
    //
    // # Arguments
    //
    // * `name` - The name of the model
    // * `repository` - The repository associated with the model
    // * `version` - The version of the model
    // * `sample` - Whether to sample data or not, Default is true
    // * `sample_size` - The sample size
    // * `feature_map` - The feature map to use
    // * `targets` - The targets to monitor
    // * `alert_config` - The alerting configuration to use
    //
    #[allow(clippy::too_many_arguments)]
    pub fn update_config_args(
        &mut self,
        name: Option<String>,
        repository: Option<String>,
        version: Option<String>,
        sample: Option<bool>,
        sample_size: Option<usize>,
        feature_map: Option<FeatureMap>,
        targets: Option<Vec<String>>,
        alert_config: Option<SpcAlertConfig>,
    ) -> Result<(), ScouterError> {
        self.config.update_config_args(
            name,
            repository,
            version,
            sample,
            sample_size,
            feature_map,
            targets,
            alert_config,
        )
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeatureMap {
    #[pyo3(get)]
    pub features: HashMap<String, HashMap<String, usize>>,
}

#[pymethods]
impl FeatureMap {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
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
pub struct SpcFeatureDrift {
    #[pyo3(get)]
    pub samples: Vec<f64>,

    #[pyo3(get)]
    pub drift: Vec<f64>,
}

impl SpcFeatureDrift {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        serde_json::to_string_pretty(&self).unwrap()
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpcDriftServerRecord {
    #[pyo3(get)]
    pub created_at: chrono::NaiveDateTime,

    #[pyo3(get)]
    pub name: String,

    #[pyo3(get)]
    pub repository: String,

    #[pyo3(get)]
    pub version: String,

    #[pyo3(get)]
    pub feature: String,

    #[pyo3(get)]
    pub value: f64,
}

#[pymethods]
impl SpcDriftServerRecord {
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

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpcDriftServerRecords {
    #[pyo3(get)]
    pub drift_type: DriftType,

    #[pyo3(get)]
    pub records: Vec<SpcDriftServerRecord>,
}

#[pymethods]
impl SpcDriftServerRecords {
    #[new]
    pub fn new(records: Vec<SpcDriftServerRecord>) -> Self {
        Self {
            drift_type: DriftType::SPC,
            records,
        }
    }
    pub fn model_dump_json(&self) -> String {
        // serialize records to a string
        ProfileFuncs::__json__(self.records.clone())
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
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
pub struct SpcDriftMap {
    #[pyo3(get)]
    pub features: HashMap<String, SpcFeatureDrift>,

    #[pyo3(get)]
    pub name: String,

    #[pyo3(get)]
    pub repository: String,

    #[pyo3(get)]
    pub version: String,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl SpcDriftMap {
    #[new]
    pub fn new(name: String, repository: String, version: String) -> Self {
        Self {
            features: HashMap::new(),
            name,
            repository,
            version,
        }
    }

    pub fn add_feature(&mut self, feature: String, profile: SpcFeatureDrift) {
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
    pub fn model_validate_json(json_string: String) -> SpcDriftMap {
        // deserialize the string to a struct
        serde_json::from_str(&json_string)
            .map_err(|_| ScouterError::DeSerializeError)
            .unwrap()
    }

    pub fn save_to_json(&self, path: Option<PathBuf>) -> Result<(), ScouterError> {
        ProfileFuncs::save_to_json(self, path, FileName::SpcDrift.to_str())
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

impl SpcDriftMap {
    pub fn to_array(&self) -> Result<ArrayReturn, ScouterError> {
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
pub struct SpcFeatureAlert {
    #[pyo3(get)]
    pub feature: String,

    #[pyo3(get)]
    pub alerts: Vec<SpcAlert>,
}

impl SpcFeatureAlert {
    pub fn new(feature: String) -> Self {
        Self {
            feature,
            alerts: Vec::new(),
        }
    }
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl SpcFeatureAlert {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpcFeatureAlerts {
    #[pyo3(get)]
    pub features: HashMap<String, SpcFeatureAlert>,

    #[pyo3(get)]
    pub has_alerts: bool,
}

impl SpcFeatureAlerts {
    // rust-only function to insert feature alerts
    pub fn insert_feature_alert(&mut self, feature: &str, alerts: &HashSet<SpcAlert>) {
        let mut feature_alert = SpcFeatureAlert::new(feature.to_string());

        // insert the alerts and indices into the feature alert
        alerts.iter().for_each(|alert| {
            feature_alert.alerts.push(SpcAlert {
                zone: alert.zone.clone(),
                kind: alert.kind.clone(),
            })
        });

        self.features.insert(feature.to_string(), feature_alert);
    }
}

impl DispatchAlertDescription for SpcFeatureAlerts {
    fn create_alert_description(&self, dispatch_type: AlertDispatchType) -> String {
        let mut alert_description = String::new();

        for (i, (_, feature_alert)) in self.features.iter().enumerate() {
            if feature_alert.alerts.is_empty() {
                continue;
            }
            if i == 0 {
                let header = match dispatch_type {
                    AlertDispatchType::Console => "Features that have drifted: \n",
                    AlertDispatchType::OpsGenie => {
                        "Drift has been detected for the following features:\n"
                    }
                    AlertDispatchType::Slack => {
                        "Drift has been detected for the following features:\n"
                    }
                };
                alert_description.push_str(header);
            }

            let feature_name = match dispatch_type {
                AlertDispatchType::Console | AlertDispatchType::OpsGenie => {
                    format!("{:indent$}{}: \n", "", &feature_alert.feature, indent = 4)
                }
                AlertDispatchType::Slack => format!("{}: \n", &feature_alert.feature),
            };

            alert_description = format!("{}{}", alert_description, feature_name);
            feature_alert.alerts.iter().for_each(|alert| {
                let alert_details = match dispatch_type {
                    AlertDispatchType::Console | AlertDispatchType::OpsGenie => {
                        let kind = format!("{:indent$}Kind: {}\n", "", &alert.kind, indent = 8);
                        let zone = format!("{:indent$}Zone: {}\n", "", &alert.zone, indent = 8);
                        format!("{}{}", kind, zone)
                    }
                    AlertDispatchType::Slack => format!(
                        "{:indent$}{} error in {}\n",
                        "",
                        &alert.kind,
                        &alert.zone,
                        indent = 4
                    ),
                };
                alert_description = format!("{}{}", alert_description, alert_details);
            });
        }
        alert_description
    }
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl SpcFeatureAlerts {
    #[new]
    pub fn new(has_alerts: bool) -> Self {
        Self {
            features: HashMap::new(),
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
        let control_alert = SpcAlertRule::default().rule;

        assert_eq!(control_alert, "8 16 4 8 2 4 1 1");
        assert_eq!(AlertZone::NotApplicable.to_str(), "NA");
        assert_eq!(AlertZone::Zone1.to_str(), "Zone 1");
        assert_eq!(AlertZone::Zone2.to_str(), "Zone 2");
        assert_eq!(AlertZone::Zone3.to_str(), "Zone 3");
        assert_eq!(AlertZone::Zone4.to_str(), "Zone 4");
        assert_eq!(SpcAlertType::AllGood.to_str(), "All good");
        assert_eq!(SpcAlertType::Consecutive.to_str(), "Consecutive");
        assert_eq!(SpcAlertType::Alternating.to_str(), "Alternating");
        assert_eq!(SpcAlertType::OutOfBounds.to_str(), "Out of bounds");
    }

    #[test]
    fn test_alert_config() {
        //test console alert config
        let alert_config = SpcAlertConfig::new(None, None, None, None, None);
        assert_eq!(alert_config.dispatch_type, AlertDispatchType::Console);
        assert_eq!(alert_config.dispatch_type(), "Console");
        assert_eq!(AlertDispatchType::Console.value(), "Console");

        //test slack alert config
        let alert_config =
            SpcAlertConfig::new(None, Some(AlertDispatchType::Slack), None, None, None);
        assert_eq!(alert_config.dispatch_type, AlertDispatchType::Slack);
        assert_eq!(alert_config.dispatch_type(), "Slack");
        assert_eq!(AlertDispatchType::Slack.value(), "Slack");

        //test opsgenie alert config
        let mut alert_kwargs = HashMap::new();
        alert_kwargs.insert("channel".to_string(), "test".to_string());

        let alert_config = SpcAlertConfig::new(
            None,
            Some(AlertDispatchType::OpsGenie),
            None,
            None,
            Some(alert_kwargs),
        );
        assert_eq!(alert_config.dispatch_type, AlertDispatchType::OpsGenie);
        assert_eq!(alert_config.dispatch_type(), "OpsGenie");
        assert_eq!(alert_config.dispatch_kwargs.get("channel").unwrap(), "test");
        assert_eq!(AlertDispatchType::OpsGenie.value(), "OpsGenie");
    }

    #[test]
    fn test_drift_config() {
        let mut drift_config =
            SpcDriftConfig::new(None, None, None, None, None, None, None, None, None).unwrap();
        assert_eq!(drift_config.sample_size, 25);
        assert!(drift_config.sample);
        assert_eq!(drift_config.name, "__missing__");
        assert_eq!(drift_config.repository, "__missing__");
        assert_eq!(drift_config.version, "0.1.0");
        assert_eq!(drift_config.targets.len(), 0);
        assert_eq!(drift_config.alert_config, SpcAlertConfig::default());

        // update
        drift_config
            .update_config_args(
                Some("test".to_string()),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();

        assert_eq!(drift_config.name, "test");
    }

    #[test]
    fn test_spc_feature_alerts() {
        // Create a sample SpcFeatureAlert (assuming SpcFeatureAlert is defined elsewhere)
        let sample_alert = SpcFeatureAlert {
            feature: "feature1".to_string(),
            alerts: vec![SpcAlert {
                kind: "kind1".to_string(),
                zone: "zone1".to_string(),
            }],
            // Initialize fields of SpcFeatureAlert
        };

        // Create a HashMap with sample data
        let mut features = HashMap::new();
        features.insert("feature1".to_string(), sample_alert.clone());

        // Create an instance of SpcFeatureAlerts
        let alerts = SpcFeatureAlerts {
            features: features.clone(),
            has_alerts: true,
        };

        // Assert the values
        assert!(alerts.has_alerts);

        // Assert the values of the features
        assert_eq!(alerts.features["feature1"].feature, sample_alert.feature);

        // Assert constructing alert description
        let _ = alerts.create_alert_description(AlertDispatchType::Console);
        let _ = alerts.create_alert_description(AlertDispatchType::OpsGenie);
        let _ = alerts.create_alert_description(AlertDispatchType::Slack);
    }
}
