use crate::core::cron::EveryDay;
use crate::core::dispatch::types::AlertDispatchType;
use crate::core::drift::base::{
    DispatchAlertDescription, DispatchDriftConfig, DriftArgs, DriftType, ProfileArgs,
    ProfileBaseArgs, RecordType, ValidateAlertConfig, MISSING,
};
use crate::core::error::ScouterError;
use crate::core::utils::{json_to_pyobject, pyobject_to_json, FeatureMap, FileName, ProfileFuncs};
use pyo3::types::PyDict;
use pyo3::{pyclass, pymethods, Bound, Py, PyResult, Python};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::debug;

// lower_threshold float,
// upper_threshold float,

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PsiServerRecord {
    #[pyo3(get)]
    pub created_at: chrono::NaiveDateTime,

    #[pyo3(get)]
    pub repository: String,

    #[pyo3(get)]
    pub name: String,

    #[pyo3(get)]
    pub version: String,

    #[pyo3(get)]
    pub feature: String,

    #[pyo3(get)]
    pub bin_id: String,

    #[pyo3(get)]
    pub bin_count: usize,

    #[pyo3(get)]
    pub record_type: RecordType,
}

#[pymethods]
impl PsiServerRecord {
    #[new]
    pub fn new(
        repository: String,
        name: String,
        version: String,
        feature: String,
        bin_id: String,
        bin_count: usize,
    ) -> Self {
        Self {
            created_at: chrono::Utc::now().naive_utc(),
            name,
            repository,
            version,
            feature,
            bin_id,
            bin_count,
            record_type: RecordType::Psi,
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
        record.insert("bind_id".to_string(), self.bin_id.clone());
        record
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct PsiAlertConfig {
    pub dispatch_type: AlertDispatchType,

    #[pyo3(get, set)]
    pub schedule: String,

    #[pyo3(get, set)]
    pub features_to_monitor: Vec<String>,

    #[pyo3(get, set)]
    pub dispatch_kwargs: HashMap<String, String>,

    #[pyo3(get, set)]
    pub psi_threshold: f64,
}

impl Default for PsiAlertConfig {
    fn default() -> PsiAlertConfig {
        Self {
            dispatch_type: AlertDispatchType::default(),
            schedule: EveryDay::new().cron,
            features_to_monitor: Vec::new(),
            dispatch_kwargs: HashMap::new(),
            psi_threshold: 0.25,
        }
    }
}

impl ValidateAlertConfig for PsiAlertConfig {}

#[pymethods]
impl PsiAlertConfig {
    #[new]
    pub fn new(
        dispatch_type: Option<AlertDispatchType>,
        schedule: Option<String>,
        features_to_monitor: Option<Vec<String>>,
        dispatch_kwargs: Option<HashMap<String, String>>,
        psi_threshold: Option<f64>,
    ) -> Self {
        let schedule = Self::resolve_schedule(schedule);
        let dispatch_type = dispatch_type.unwrap_or_default();
        let features_to_monitor = features_to_monitor.unwrap_or_default();
        let dispatch_kwargs = dispatch_kwargs.unwrap_or_default();
        let psi_threshold = psi_threshold.unwrap_or(0.25);

        Self {
            dispatch_type,
            schedule,
            features_to_monitor,
            dispatch_kwargs,
            psi_threshold,
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

// TODO dry this out
#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PsiDriftConfig {
    #[pyo3(get, set)]
    pub repository: String,

    #[pyo3(get, set)]
    pub name: String,

    #[pyo3(get, set)]
    pub version: String,

    #[pyo3(get, set)]
    pub feature_map: FeatureMap,

    #[pyo3(get, set)]
    pub alert_config: PsiAlertConfig,

    #[pyo3(get, set)]
    pub targets: Vec<String>,

    #[pyo3(get, set)]
    pub drift_type: DriftType,
}

#[pymethods]
#[allow(clippy::too_many_arguments)]
impl PsiDriftConfig {
    // TODO dry this out
    #[new]
    #[pyo3(signature = (repository=None, name=None, version=None, feature_map=None, targets=None, alert_config=None, config_path=None))]
    pub fn new(
        repository: Option<String>,
        name: Option<String>,
        version: Option<String>,
        feature_map: Option<FeatureMap>,
        targets: Option<Vec<String>>,
        alert_config: Option<PsiAlertConfig>,
        config_path: Option<PathBuf>,
    ) -> Result<Self, ScouterError> {
        if let Some(config_path) = config_path {
            let config = PsiDriftConfig::load_from_json_file(config_path);
            return config;
        }

        let name = name.unwrap_or(MISSING.to_string());
        let repository = repository.unwrap_or(MISSING.to_string());

        if name == MISSING || repository == MISSING {
            debug!("Name and repository were not provided. Defaulting to __missing__");
        }

        let version = version.unwrap_or("0.1.0".to_string());
        let targets = targets.unwrap_or_default();
        let alert_config = alert_config.unwrap_or_default();
        let feature_map = feature_map.unwrap_or_default();

        Ok(Self {
            name,
            repository,
            version,
            alert_config,
            feature_map,
            targets,
            drift_type: DriftType::Psi,
        })
    }

    #[staticmethod]
    pub fn load_from_json_file(path: PathBuf) -> Result<PsiDriftConfig, ScouterError> {
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

    pub fn update_feature_map(&mut self, feature_map: FeatureMap) {
        self.feature_map = feature_map;
    }

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (repository=None, name=None, version=None, targets=None, alert_config=None))]
    pub fn update_config_args(
        &mut self,
        repository: Option<String>,
        name: Option<String>,
        version: Option<String>,
        targets: Option<Vec<String>>,
        alert_config: Option<PsiAlertConfig>,
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

// TODO dry this out
impl DispatchDriftConfig for PsiDriftConfig {
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
pub struct Bin {
    #[pyo3(get)]
    pub id: String,

    #[pyo3(get)]
    pub lower_limit: Option<f64>,

    #[pyo3(get)]
    pub upper_limit: Option<f64>,

    #[pyo3(get)]
    pub proportion: f64,
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PsiFeatureDriftProfile {
    #[pyo3(get)]
    pub id: String,

    #[pyo3(get)]
    pub bins: Vec<Bin>,

    #[pyo3(get)]
    pub timestamp: chrono::NaiveDateTime,
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PsiDriftProfile {
    #[pyo3(get, set)]
    pub features: HashMap<String, PsiFeatureDriftProfile>,

    #[pyo3(get, set)]
    pub config: PsiDriftConfig,

    #[pyo3(get, set)]
    pub scouter_version: String,
}

#[pymethods]
impl PsiDriftProfile {
    #[new]
    #[pyo3(signature = (features, config, scouter_version=None))]
    pub fn new(
        features: HashMap<String, PsiFeatureDriftProfile>,
        config: PsiDriftConfig,
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
    // TODO dry this out
    pub fn model_dump(&self, py: Python) -> PyResult<Py<PyDict>> {
        let json_str = serde_json::to_string(&self).map_err(|_| ScouterError::SerializeError)?;

        let json_value: Value =
            serde_json::from_str(&json_str).map_err(|_| ScouterError::DeSerializeError)?;

        // Create a new Python dictionary
        let dict = PyDict::new(py);

        // Convert JSON to Python dict
        json_to_pyobject(py, &json_value, &dict)?;

        // Return the Python dictionary
        Ok(dict.into())
    }

    #[staticmethod]
    pub fn model_validate(data: &Bound<'_, PyDict>) -> PsiDriftProfile {
        let json_value = pyobject_to_json(data).unwrap();

        let string = serde_json::to_string(&json_value).unwrap();
        serde_json::from_str(&string).expect("Failed to load drift profile")
    }

    #[staticmethod]
    pub fn model_validate_json(json_string: String) -> PsiDriftProfile {
        // deserialize the string to a struct
        serde_json::from_str(&json_string).expect("Failed to load monitor profile")
    }

    // Convert python dict into a drift profile
    pub fn save_to_json(&self, path: Option<PathBuf>) -> Result<(), ScouterError> {
        ProfileFuncs::save_to_json(self, path, FileName::Profile.to_str())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_config_args(
        &mut self,
        repository: Option<String>,
        name: Option<String>,
        version: Option<String>,
        targets: Option<Vec<String>>,
        alert_config: Option<PsiAlertConfig>,
    ) -> Result<(), ScouterError> {
        self.config
            .update_config_args(repository, name, version, targets, alert_config)
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PsiDriftMap {
    #[pyo3(get)]
    pub features: HashMap<String, f64>,

    #[pyo3(get)]
    pub name: String,

    #[pyo3(get)]
    pub repository: String,

    #[pyo3(get)]
    pub version: String,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl PsiDriftMap {
    #[new]
    pub fn new(repository: String, name: String, version: String) -> Self {
        Self {
            features: HashMap::new(),
            name,
            repository,
            version,
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
    pub fn model_validate_json(json_string: String) -> PsiDriftMap {
        // deserialize the string to a struct
        serde_json::from_str(&json_string)
            .map_err(|_| ScouterError::DeSerializeError)
            .unwrap()
    }

    pub fn save_to_json(&self, path: Option<PathBuf>) -> Result<(), ScouterError> {
        ProfileFuncs::save_to_json(self, path, FileName::PsiDrift.to_str())
    }
}

// TODO dry this out
impl ProfileBaseArgs for PsiDriftProfile {
    /// Get the base arguments for the profile (convenience method on the server)
    fn get_base_args(&self) -> ProfileArgs {
        ProfileArgs {
            name: self.config.name.clone(),
            repository: self.config.repository.clone(),
            version: self.config.version.clone(),
            schedule: self.config.alert_config.schedule.clone(),
            scouter_version: self.scouter_version.clone(),
            drift_type: self.config.drift_type.clone(),
        }
    }

    /// Convert the struct to a serde_json::Value
    fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PsiFeatureAlerts {
    #[pyo3(get)]
    pub features: HashMap<String, f64>,

    #[pyo3(get)]
    pub threshold: f64,
}

impl DispatchAlertDescription for PsiFeatureAlerts {
    fn create_alert_description(&self, dispatch_type: AlertDispatchType) -> String {
        let mut alert_description = String::new();

        for (i, (feature_name, drift_value)) in self.features.iter().enumerate() {
            let description = format!("Feature '{}' has experienced drift, with a current PSI score of {} that exceeds the configured threshold of {}.", feature_name, drift_value, self.threshold);

            if i == 0 {
                let header = "PSI Drift has been detected for the following features:\n";
                alert_description.push_str(header);
            }

            let feature_name = match dispatch_type {
                AlertDispatchType::Console | AlertDispatchType::OpsGenie => {
                    format!("{:indent$}{}: \n", "", &feature_name, indent = 4)
                }
                AlertDispatchType::Slack => format!("{}: \n", &feature_name),
            };

            alert_description = format!("{}{}", alert_description, feature_name);

            let alert_details = match dispatch_type {
                AlertDispatchType::Console | AlertDispatchType::OpsGenie => {
                    format!("{:indent$}Drift Value: {}\n", "", description, indent = 8)
                }
                AlertDispatchType::Slack => {
                    format!("{:indent$}Drift Value: {}\n", "", description, indent = 4)
                }
            };
            alert_description = format!("{}{}", alert_description, alert_details);
        }
        alert_description
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_config() {
        //test console alert config
        let alert_config = PsiAlertConfig::new(None, None, None, None, None);
        assert_eq!(alert_config.dispatch_type, AlertDispatchType::Console);
        assert_eq!(alert_config.dispatch_type(), "Console");
        assert_eq!(AlertDispatchType::Console.value(), "Console");

        //test slack alert config
        let alert_config =
            PsiAlertConfig::new(Some(AlertDispatchType::Slack), None, None, None, None);
        assert_eq!(alert_config.dispatch_type, AlertDispatchType::Slack);
        assert_eq!(alert_config.dispatch_type(), "Slack");
        assert_eq!(AlertDispatchType::Slack.value(), "Slack");

        //test opsgenie alert config
        let mut alert_kwargs = HashMap::new();
        alert_kwargs.insert("channel".to_string(), "test".to_string());

        let alert_config = PsiAlertConfig::new(
            Some(AlertDispatchType::OpsGenie),
            None,
            None,
            Some(alert_kwargs),
            None,
        );
        assert_eq!(alert_config.dispatch_type, AlertDispatchType::OpsGenie);
        assert_eq!(alert_config.dispatch_type(), "OpsGenie");
        assert_eq!(alert_config.dispatch_kwargs.get("channel").unwrap(), "test");
        assert_eq!(AlertDispatchType::OpsGenie.value(), "OpsGenie");
    }

    #[test]
    fn test_drift_config() {
        let mut drift_config =
            PsiDriftConfig::new(None, None, None, None, None, None, None).unwrap();
        assert_eq!(drift_config.name, "__missing__");
        assert_eq!(drift_config.repository, "__missing__");
        assert_eq!(drift_config.version, "0.1.0");
        assert_eq!(drift_config.targets.len(), 0);
        assert_eq!(drift_config.alert_config, PsiAlertConfig::default());

        // update
        drift_config
            .update_config_args(None, Some("test".to_string()), None, None, None)
            .unwrap();

        assert_eq!(drift_config.name, "test");
    }

    #[test]
    fn test_create_alert_description() {
        let features = HashMap::from([
            ("feature1".to_string(), 0.35),
            ("feature2".to_string(), 0.45),
        ]);
        let threshold = 0.3;
        let psi_feature_alerts = PsiFeatureAlerts {
            features,
            threshold,
        };

        // Test for Console dispatch type
        let description = psi_feature_alerts.create_alert_description(AlertDispatchType::Console);
        assert!(description.contains("PSI Drift has been detected for the following features:"));
        assert!(description.contains("Feature 'feature1' has experienced drift, with a current PSI score of 0.35 that exceeds the configured threshold of 0.3."));
        assert!(description.contains("Feature 'feature2' has experienced drift, with a current PSI score of 0.45 that exceeds the configured threshold of 0.3."));

        // Test for Slack dispatch type
        let description = psi_feature_alerts.create_alert_description(AlertDispatchType::Slack);
        assert!(description.contains("PSI Drift has been detected for the following features:"));
        assert!(description.contains("Feature 'feature1' has experienced drift, with a current PSI score of 0.35 that exceeds the configured threshold of 0.3."));
        assert!(description.contains("Feature 'feature2' has experienced drift, with a current PSI score of 0.45 that exceeds the configured threshold of 0.3."));

        // Test for OpsGenie dispatch type
        let description = psi_feature_alerts.create_alert_description(AlertDispatchType::OpsGenie);
        assert!(description.contains("PSI Drift has been detected for the following features:"));
        assert!(description.contains("Feature 'feature1' has experienced drift, with a current PSI score of 0.35 that exceeds the configured threshold of 0.3."));
        assert!(description.contains("Feature 'feature2' has experienced drift, with a current PSI score of 0.45 that exceeds the configured threshold of 0.3."));
    }
}
