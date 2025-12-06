#![allow(clippy::useless_conversion)]
use crate::binning::equal_width::EqualWidthBinning;
use crate::binning::quantile::QuantileBinning;
use crate::binning::strategy::BinningStrategy;
use crate::error::{ProfileError, TypeError};
use crate::psi::alert::PsiAlertConfig;
use crate::util::{json_to_pyobject, pyobject_to_json, scouter_version, ConfigExt};
use crate::ProfileRequest;
use crate::VersionRequest;
use crate::{
    DispatchDriftConfig, DriftArgs, DriftType, FeatureMap, FileName, ProfileArgs, ProfileBaseArgs,
    PyHelperFuncs, DEFAULT_VERSION, MISSING,
};
use chrono::Utc;
use core::fmt::Debug;
use potato_head::create_uuid7;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use scouter_semver::VersionType;
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
    Numeric,
    Category,
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PsiDriftConfig {
    #[pyo3(get, set)]
    pub space: String,

    #[pyo3(get, set)]
    pub name: String,

    #[pyo3(get, set)]
    pub version: String,

    #[pyo3(get)]
    pub uid: String,

    #[pyo3(get, set)]
    pub alert_config: PsiAlertConfig,

    #[pyo3(get)]
    #[serde(default)]
    pub feature_map: FeatureMap,

    #[pyo3(get)]
    #[serde(default = "default_drift_type")]
    pub drift_type: DriftType,

    #[pyo3(get, set)]
    pub categorical_features: Option<Vec<String>>,

    pub binning_strategy: BinningStrategy,
}

impl ConfigExt for PsiDriftConfig {
    fn space(&self) -> &str {
        &self.space
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        &self.version
    }
}

fn default_drift_type() -> DriftType {
    DriftType::Psi
}

impl PsiDriftConfig {
    pub fn update_feature_map(&mut self, feature_map: FeatureMap) {
        self.feature_map = feature_map;
    }
}

#[pymethods]
#[allow(clippy::too_many_arguments)]
impl PsiDriftConfig {
    #[new]
    #[pyo3(signature = (space=MISSING, name=MISSING, version=DEFAULT_VERSION, alert_config=PsiAlertConfig::default(), config_path=None, categorical_features=None, binning_strategy=None))]
    pub fn new(
        space: &str,
        name: &str,
        version: &str,
        alert_config: PsiAlertConfig,
        config_path: Option<PathBuf>,
        categorical_features: Option<Vec<String>>,
        binning_strategy: Option<&Bound<'_, PyAny>>,
    ) -> Result<Self, ProfileError> {
        if let Some(config_path) = config_path {
            let config = PsiDriftConfig::load_from_json_file(config_path);
            return config;
        }

        let binning_strategy = match binning_strategy {
            None => BinningStrategy::default(),
            Some(strategy) => {
                if strategy.is_instance_of::<QuantileBinning>() {
                    BinningStrategy::QuantileBinning(strategy.extract()?)
                } else if strategy.is_instance_of::<EqualWidthBinning>() {
                    BinningStrategy::EqualWidthBinning(strategy.extract()?)
                } else {
                    return Err(ProfileError::InvalidBinningStrategyError);
                }
            }
        };

        if name == MISSING || space == MISSING {
            debug!("Name and space were not provided. Defaulting to __missing__");
        }

        Ok(Self {
            name: name.to_string(),
            space: space.to_string(),
            version: version.to_string(),
            uid: create_uuid7(),
            alert_config,
            categorical_features,
            feature_map: FeatureMap::default(),
            drift_type: DriftType::Psi,
            binning_strategy,
        })
    }

    #[staticmethod]
    pub fn load_from_json_file(path: PathBuf) -> Result<PsiDriftConfig, ProfileError> {
        // deserialize the string to a struct

        let file = std::fs::read_to_string(&path)?;

        Ok(serde_json::from_str(&file)?)
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__json__(self)
    }

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (space=None, name=None, version=None, uid=None, alert_config=None, categorical_features=None, binning_strategy=None))]
    pub fn update_config_args(
        &mut self,
        space: Option<String>,
        name: Option<String>,
        version: Option<String>,
        uid: Option<String>,
        alert_config: Option<PsiAlertConfig>,
        categorical_features: Option<Vec<String>>,
        binning_strategy: Option<&Bound<'_, PyAny>>,
    ) -> Result<(), TypeError> {
        if name.is_some() {
            self.name = name.ok_or(TypeError::MissingNameError)?;
        }

        if space.is_some() {
            self.space = space.ok_or(TypeError::MissingSpaceError)?;
        }

        if version.is_some() {
            self.version = version.ok_or(TypeError::MissingVersionError)?;
        }

        if alert_config.is_some() {
            self.alert_config = alert_config.ok_or(TypeError::MissingAlertConfigError)?;
        }

        if uid.is_some() {
            self.uid = uid.ok_or(TypeError::MissingUidError)?;
        }

        if categorical_features.is_some() {
            self.categorical_features = categorical_features;
        }

        if let Some(binning_strategy) = binning_strategy {
            if binning_strategy.is_instance_of::<QuantileBinning>() {
                self.binning_strategy =
                    BinningStrategy::QuantileBinning(binning_strategy.extract()?);
            } else if binning_strategy.is_instance_of::<EqualWidthBinning>() {
                self.binning_strategy =
                    BinningStrategy::EqualWidthBinning(binning_strategy.extract()?);
            } else {
                return Err(TypeError::InvalidBinningStrategyError);
            }
        }

        Ok(())
    }

    #[getter]
    pub fn binning_strategy<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.binning_strategy.strategy(py)
    }

    #[setter]
    pub fn set_binning_strategy(&mut self, strategy: &Bound<'_, PyAny>) -> PyResult<()> {
        if strategy.is_instance_of::<QuantileBinning>() {
            self.binning_strategy = BinningStrategy::QuantileBinning(strategy.extract()?);
        } else if strategy.is_instance_of::<EqualWidthBinning>() {
            self.binning_strategy = BinningStrategy::EqualWidthBinning(strategy.extract()?);
        } else {
            return Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                "Invalid binning strategy type",
            ));
        }
        Ok(())
    }
}

impl Default for PsiDriftConfig {
    fn default() -> Self {
        PsiDriftConfig {
            name: "__missing__".to_string(),
            space: "__missing__".to_string(),
            version: DEFAULT_VERSION.to_string(),
            uid: MISSING.to_string(),
            feature_map: FeatureMap::default(),
            alert_config: PsiAlertConfig::default(),
            drift_type: DriftType::Psi,
            categorical_features: None,
            binning_strategy: BinningStrategy::default(),
        }
    }
}
// TODO dry this out

impl DispatchDriftConfig for PsiDriftConfig {
    fn get_drift_args(&self) -> DriftArgs {
        DriftArgs {
            name: self.name.clone(),
            space: self.space.clone(),
            version: self.version.clone(),
            dispatch_config: self.alert_config.dispatch_config.clone(),
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
    pub timestamp: chrono::DateTime<Utc>,

    #[pyo3(get)]
    pub bin_type: BinType,
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PsiDriftProfile {
    #[pyo3(get)]
    pub features: HashMap<String, PsiFeatureDriftProfile>,

    #[pyo3(get)]
    pub config: PsiDriftConfig,

    #[pyo3(get)]
    pub scouter_version: String,
}

impl PsiDriftProfile {
    pub fn new(features: HashMap<String, PsiFeatureDriftProfile>, config: PsiDriftConfig) -> Self {
        Self {
            features,
            config,
            scouter_version: scouter_version(),
        }
    }
}

#[pymethods]
impl PsiDriftProfile {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }

    #[getter]
    pub fn uid(&self) -> String {
        self.config.uid.clone()
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__json__(self)
    }
    // TODO dry this out
    #[allow(clippy::useless_conversion)]
    pub fn model_dump(&self, py: Python) -> Result<Py<PyDict>, ProfileError> {
        let json_str = serde_json::to_string(&self)?;

        let json_value: Value = serde_json::from_str(&json_str)?;

        // Create a new Python dictionary
        let dict = PyDict::new(py);

        // Convert JSON to Python dict
        json_to_pyobject(py, &json_value, &dict)?;

        // Return the Python dictionary
        Ok(dict.into())
    }

    #[staticmethod]
    pub fn from_file(path: PathBuf) -> Result<PsiDriftProfile, ProfileError> {
        let file = std::fs::read_to_string(&path)?;

        Ok(serde_json::from_str(&file)?)
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
    pub fn save_to_json(&self, path: Option<PathBuf>) -> Result<PathBuf, ProfileError> {
        Ok(PyHelperFuncs::save_to_json(
            self,
            path,
            FileName::PsiDriftProfile.to_str(),
        )?)
    }

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (space=None, name=None, version=None, uid=None,alert_config=None, categorical_features=None, binning_strategy=None))]
    pub fn update_config_args(
        &mut self,
        space: Option<String>,
        name: Option<String>,
        version: Option<String>,
        uid: Option<String>,
        alert_config: Option<PsiAlertConfig>,
        categorical_features: Option<Vec<String>>,
        binning_strategy: Option<&Bound<'_, PyAny>>,
    ) -> Result<(), TypeError> {
        self.config.update_config_args(
            space,
            name,
            version,
            uid,
            alert_config,
            categorical_features,
            binning_strategy,
        )
    }

    /// Create a profile request from the profile
    pub fn create_profile_request(&self) -> Result<ProfileRequest, TypeError> {
        let version: Option<String> = if self.config.version == DEFAULT_VERSION {
            None
        } else {
            Some(self.config.version.clone())
        };

        Ok(ProfileRequest {
            space: self.config.space.clone(),
            profile: self.model_dump_json(),
            drift_type: self.config.drift_type.clone(),
            version_request: Some(VersionRequest {
                version,
                version_type: VersionType::Minor,
                pre_tag: None,
                build_tag: None,
            }),
            active: false,
            deactivate_others: false,
        })
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
    pub space: String,

    #[pyo3(get)]
    pub version: String,
}

impl PsiDriftMap {
    pub fn new(space: String, name: String, version: String) -> Self {
        Self {
            features: HashMap::new(),
            name,
            space,
            version,
        }
    }
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl PsiDriftMap {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__json__(self)
    }

    #[staticmethod]
    pub fn model_validate_json(json_string: String) -> Result<PsiDriftMap, ProfileError> {
        // deserialize the string to a struct
        Ok(serde_json::from_str(&json_string)?)
    }

    #[pyo3(signature = (path=None))]
    pub fn save_to_json(&self, path: Option<PathBuf>) -> Result<PathBuf, ProfileError> {
        Ok(PyHelperFuncs::save_to_json(
            self,
            path,
            FileName::PsiDriftMap.to_str(),
        )?)
    }
}

// TODO dry this out
impl ProfileBaseArgs for PsiDriftProfile {
    type Config = PsiDriftConfig;

    fn config(&self) -> &Self::Config {
        &self.config
    }
    /// Get the base arguments for the profile (convenience method on the server)
    fn get_base_args(&self) -> ProfileArgs {
        ProfileArgs {
            name: self.config.name.clone(),
            space: self.config.space.clone(),
            version: Some(self.config.version.clone()),
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
pub struct DistributionData {
    pub sample_size: u64,
    pub bins: BTreeMap<usize, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureDistributions {
    pub distributions: BTreeMap<String, DistributionData>,
}

impl FeatureDistributions {
    pub fn is_empty(&self) -> bool {
        self.distributions.is_empty()
    }
}

#[derive(Debug)]
pub struct FeatureDistributionRow {
    pub feature: String,
    pub distribution: DistributionData,
}

#[cfg(feature = "server")]
impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for FeatureDistributionRow {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;

        let feature: String = row.try_get("feature")?;
        let sample_size: i64 = row.try_get("sample_size")?;
        let bins_json: serde_json::Value = row.try_get("bins")?;
        let bins: BTreeMap<usize, f64> =
            serde_json::from_value(bins_json).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;

        Ok(FeatureDistributionRow {
            feature,
            distribution: DistributionData {
                sample_size: sample_size as u64,
                bins,
            },
        })
    }
}

impl FeatureDistributions {
    /// Convert from a vector of rows into the final structure
    pub fn from_rows(rows: Vec<FeatureDistributionRow>) -> Self {
        let distributions = rows
            .into_iter()
            .map(|row| (row.feature, row.distribution))
            .collect();

        FeatureDistributions { distributions }
    }
}
