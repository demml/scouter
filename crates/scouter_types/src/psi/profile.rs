use crate::psi::alert::PsiAlertConfig;
use crate::util::{json_to_pyobject, pyobject_to_json};
use crate::{
    DispatchDriftConfig, DriftArgs, DriftType, FeatureMap, FileName, ProfileArgs, ProfileBaseArgs,
    ProfileFuncs, MISSING,
};
use core::fmt::Debug;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use scouter_error::ScouterError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::debug;

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
    #[pyo3(signature = (path=None))]
    pub fn save_to_json(&self, path: Option<PathBuf>) -> Result<(), ScouterError> {
        ProfileFuncs::save_to_json(self, path, FileName::Profile.to_str())
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

    #[pyo3(signature = (path=None))]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureBinProportion {
    pub feature: String,
    pub bin_id: String,
    pub proportion: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureBinProportions {
    pub features: HashMap<String, HashMap<String, f64>>,
}

impl FeatureBinProportions {
    pub fn from_bins(bins: Vec<FeatureBinProportion>) -> Self {
        let mut features: HashMap<String, HashMap<String, f64>> = HashMap::new();
        for bin in bins {
            let feature = features.entry(bin.feature).or_insert(HashMap::new());
            feature.insert(bin.bin_id, bin.proportion);
        }
        FeatureBinProportions { features }
    }

    pub fn get(&self, feature: &str, bin: &str) -> Option<&f64> {
        self.features.get(feature).and_then(|f| f.get(bin))
    }

    pub fn is_empty(&self) -> bool {
        self.features.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
