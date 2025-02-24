use crate::psi::alert::PsiAlertConfig;
use crate::util::{json_to_pyobject, pyobject_to_json};
use crate::{
    DispatchDriftConfig, DriftArgs, DriftType, FeatureMap, FileName, ProfileArgs, ProfileBaseArgs,
    ProfileFuncs, DEFAULT_VERSION, MISSING,
};
use core::fmt::Debug;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use scouter_error::ScouterError;
use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use tracing::debug;

#[pyclass(eq)]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum BinType {
    Binary,
    Numeric,
    Category,
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PsiDriftConfig {
    #[pyo3(get, set)]
    pub repository: String,

    #[pyo3(get, set)]
    pub name: String,

    #[pyo3(get, set)]
    pub version: String,

    #[pyo3(get)]
    pub feature_map: FeatureMap,

    #[pyo3(get, set)]
    pub alert_config: PsiAlertConfig,

    #[pyo3(get, set)]
    pub targets: Vec<String>,

    #[pyo3(get)]
    pub drift_type: DriftType,
}

#[pymethods]
#[allow(clippy::too_many_arguments)]
impl PsiDriftConfig {
    // TODO dry this out
    #[new]
    #[pyo3(signature = (repository=MISSING, name=MISSING, version=DEFAULT_VERSION, features_to_monitor=None, targets=None, alert_config=PsiAlertConfig::default(), config_path=None))]
    pub fn new(
        repository: &str,
        name: &str,
        version: &str,
        features_to_monitor: Option<Vec<String>>,
        targets: Option<Vec<String>>,
        mut alert_config: PsiAlertConfig,
        config_path: Option<PathBuf>,
    ) -> Result<Self, ScouterError> {
        if let Some(config_path) = config_path {
            let config = PsiDriftConfig::load_from_json_file(config_path);
            return config;
        }

        if name == MISSING || repository == MISSING {
            debug!("Name and repository were not provided. Defaulting to __missing__");
        }

        let targets = targets.unwrap_or_default();

        if features_to_monitor.is_some() {
            alert_config.features_to_monitor = features_to_monitor.unwrap();
        }

        Ok(Self {
            name: name.to_string(),
            repository: repository.to_string(),
            version: version.to_string(),
            alert_config,
            feature_map: FeatureMap::default(),
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

impl Default for PsiDriftConfig {
    fn default() -> Self {
        PsiDriftConfig {
            name: "__missing__".to_string(),
            repository: "__missing__".to_string(),
            version: DEFAULT_VERSION.to_string(),
            feature_map: FeatureMap::default(),
            alert_config: PsiAlertConfig::default(),
            targets: Vec::new(),
            drift_type: DriftType::Psi,
        }
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
            dispatch_kwargs: self.alert_config.dispatch_kwargs.clone(),
        }
    }
}

#[pyclass]
#[derive(Debug, Clone, PartialEq)]
pub struct Bin {
    #[pyo3(get)]
    pub id: usize,

    #[pyo3(get)]
    pub lower_limit: Option<f64>,

    #[pyo3(get)]
    pub upper_limit: Option<f64>,

    #[pyo3(get)]
    pub proportion: f64,
}

impl Serialize for Bin {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Bin", 4)?;
        state.serialize_field("id", &self.id)?;

        state.serialize_field(
            "lower_limit",
            &self.lower_limit.map(|v| {
                if v.is_infinite() {
                    serde_json::Value::String(if v.is_sign_positive() {
                        "inf".to_string()
                    } else {
                        "-inf".to_string()
                    })
                } else {
                    serde_json::Value::Number(serde_json::Number::from_f64(v).unwrap())
                }
            }),
        )?;
        state.serialize_field(
            "upper_limit",
            &self.upper_limit.map(|v| {
                if v.is_infinite() {
                    serde_json::Value::String(if v.is_sign_positive() {
                        "inf".to_string()
                    } else {
                        "-inf".to_string()
                    })
                } else {
                    serde_json::Value::Number(serde_json::Number::from_f64(v).unwrap())
                }
            }),
        )?;
        state.serialize_field("proportion", &self.proportion)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for Bin {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum NumberOrString {
            Number(f64),
            String(String),
        }

        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Id,
            LowerLimit,
            UpperLimit,
            Proportion,
        }

        struct BinVisitor;

        impl<'de> Visitor<'de> for BinVisitor {
            type Value = Bin;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct Bin")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Bin, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut id = None;
                let mut lower_limit = None;
                let mut upper_limit = None;
                let mut proportion = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Id => {
                            id = Some(map.next_value()?);
                        }
                        Field::LowerLimit => {
                            let val: Option<NumberOrString> = map.next_value()?;
                            lower_limit = Some(val.map(|v| match v {
                                NumberOrString::String(s) => match s.as_str() {
                                    "inf" => f64::INFINITY,
                                    "-inf" => f64::NEG_INFINITY,
                                    _ => s.parse().unwrap(),
                                },
                                NumberOrString::Number(n) => n,
                            }));
                        }
                        Field::UpperLimit => {
                            let val: Option<NumberOrString> = map.next_value()?;
                            upper_limit = Some(val.map(|v| match v {
                                NumberOrString::String(s) => match s.as_str() {
                                    "inf" => f64::INFINITY,
                                    "-inf" => f64::NEG_INFINITY,
                                    _ => s.parse().unwrap(),
                                },
                                NumberOrString::Number(n) => n,
                            }));
                        }
                        Field::Proportion => {
                            proportion = Some(map.next_value()?);
                        }
                    }
                }

                Ok(Bin {
                    id: id.ok_or_else(|| de::Error::missing_field("id"))?,
                    lower_limit: lower_limit
                        .ok_or_else(|| de::Error::missing_field("lower_limit"))?,
                    upper_limit: upper_limit
                        .ok_or_else(|| de::Error::missing_field("upper_limit"))?,
                    proportion: proportion.ok_or_else(|| de::Error::missing_field("proportion"))?,
                })
            }
        }

        const FIELDS: &[&str] = &["id", "lower_limit", "upper_limit", "proportion"];
        deserializer.deserialize_struct("Bin", FIELDS, BinVisitor)
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PsiFeatureDriftProfile {
    #[pyo3(get)]
    pub id: String,

    #[pyo3(get)]
    pub bins: Vec<Bin>,

    #[pyo3(get)]
    pub timestamp: chrono::NaiveDateTime,

    #[pyo3(get)]
    pub bin_type: BinType,
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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
    #[allow(clippy::useless_conversion)]
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
    pub fn from_file(path: PathBuf) -> Result<PsiDriftProfile, ScouterError> {
        let file = std::fs::read_to_string(&path).map_err(|_| ScouterError::ReadError)?;

        serde_json::from_str(&file).map_err(|_| ScouterError::DeSerializeError)
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
    pub bins: BTreeMap<usize, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureBinProportions {
    pub features: BTreeMap<String, BTreeMap<usize, f64>>,
}

impl FromIterator<FeatureBinProportion> for FeatureBinProportions {
    fn from_iter<T: IntoIterator<Item = FeatureBinProportion>>(iter: T) -> Self {
        let mut feature_map: BTreeMap<String, BTreeMap<usize, f64>> = BTreeMap::new();
        for feature in iter {
            feature_map.insert(feature.feature, feature.bins);
        }
        FeatureBinProportions {
            features: feature_map,
        }
    }
}

impl FeatureBinProportions {
    pub fn from_features(features: Vec<FeatureBinProportion>) -> Self {
        let mut feature_map: BTreeMap<String, BTreeMap<usize, f64>> = BTreeMap::new();
        for feature in features {
            feature_map.insert(feature.feature, feature.bins);
        }
        FeatureBinProportions {
            features: feature_map,
        }
    }

    pub fn get(&self, feature: &str, bin: &usize) -> Option<&f64> {
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
        let mut drift_config = PsiDriftConfig::new(
            MISSING,
            MISSING,
            DEFAULT_VERSION,
            None,
            None,
            PsiAlertConfig::default(),
            None,
        )
        .unwrap();
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
