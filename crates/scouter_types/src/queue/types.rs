use crate::error::TypeError;
use crate::json_to_pyobject_value;
use crate::util::{is_pydantic_basemodel, pyobject_to_json};
use crate::PyHelperFuncs;
use chrono::DateTime;
use chrono::Utc;
use potato_head::create_uuid7;
use potato_head::Prompt;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyFloat, PyInt, PyList, PyString};
use pyo3::IntoPyObjectExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;

#[pyclass]
#[derive(Clone, Debug, Serialize)]
pub enum EntityType {
    Feature,
    Metric,
    LLM,
}

#[pyclass]
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct IntFeature {
    pub name: String,
    pub value: i64,
}

#[pymethods]
impl IntFeature {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

impl IntFeature {
    pub fn to_float(&self) -> f64 {
        self.value as f64
    }
}

#[pyclass]
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FloatFeature {
    pub name: String,
    pub value: f64,
}

#[pymethods]
impl FloatFeature {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

#[pyclass]
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct StringFeature {
    pub name: String,
    pub value: String,
}

#[pymethods]
impl StringFeature {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

impl StringFeature {
    pub fn to_float(&self, feature_map: &FeatureMap) -> Result<f64, TypeError> {
        feature_map
            .features
            .get(&self.name)
            .and_then(|feat_map| {
                feat_map
                    .get(&self.value)
                    .or_else(|| feat_map.get("missing"))
            })
            .map(|&val| val as f64)
            .ok_or(TypeError::MissingStringValueError)
    }
}

#[pyclass(name = "QueueFeature")]
#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum Feature {
    Int(IntFeature),
    Float(FloatFeature),
    String(StringFeature),
}

#[pymethods]
impl Feature {
    #[new]
    /// Parses a value to it's corresponding feature type.
    /// PyFLoat -> FloatFeature
    /// PyInt -> IntFeature
    /// PyString -> StringFeature
    /// # Arguments
    /// * `name` - The name of the feature.
    /// * `feature` - The value of the feature, which can be a PyFloat
    /// # Returns
    /// * `Feature` - The corresponding feature type.
    /// # Errors
    /// * `TypeError` - If the feature type is not supported.
    pub fn new(name: &str, feature: Bound<'_, PyAny>) -> Result<Self, TypeError> {
        // check python type
        if feature.is_instance_of::<PyFloat>() {
            let value: f64 = feature.extract().unwrap();
            Ok(Feature::Float(FloatFeature {
                name: name.into(),
                value,
            }))
        } else if feature.is_instance_of::<PyInt>() {
            let value: i64 = feature.extract().unwrap();
            Ok(Feature::Int(IntFeature {
                name: name.into(),
                value,
            }))
        } else if feature.is_instance_of::<PyString>() {
            let value: String = feature.extract().unwrap();
            Ok(Feature::String(StringFeature {
                name: name.into(),
                value,
            }))
        } else {
            Err(TypeError::UnsupportedFeatureTypeError(
                feature.get_type().name()?.to_string(),
            ))
        }
    }

    #[staticmethod]
    pub fn int(name: String, value: i64) -> Self {
        Feature::Int(IntFeature { name, value })
    }

    #[staticmethod]
    pub fn float(name: String, value: f64) -> Self {
        Feature::Float(FloatFeature { name, value })
    }

    #[staticmethod]
    pub fn string(name: String, value: String) -> Self {
        Feature::String(StringFeature { name, value })
    }

    #[staticmethod]
    pub fn categorical(name: String, value: String) -> Self {
        Feature::String(StringFeature { name, value })
    }

    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

impl Feature {
    pub fn to_float(&self, feature_map: &FeatureMap) -> Result<f64, TypeError> {
        match self {
            Feature::Int(feature) => Ok(feature.to_float()),
            Feature::Float(feature) => Ok(feature.value),
            Feature::String(feature) => feature.to_float(feature_map),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Feature::Int(feature) => &feature.name,
            Feature::Float(feature) => &feature.name,
            Feature::String(feature) => &feature.name,
        }
    }

    pub fn to_usize(&self, feature_map: &FeatureMap) -> Result<usize, TypeError> {
        match self {
            Feature::Int(f) => Ok(f.value as usize),
            Feature::Float(f) => Ok(f.value as usize),
            Feature::String(f) => Ok(f.to_float(feature_map)? as usize),
        }
    }
}

impl Display for Feature {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Feature::Int(feature) => write!(f, "{}", feature.value),
            Feature::Float(feature) => write!(f, "{}", feature.value),
            Feature::String(feature) => write!(f, "{}", feature.value),
        }
    }
}

#[pyclass]
#[derive(Clone, Debug, Serialize)]
pub struct Features {
    #[pyo3(get)]
    pub features: Vec<Feature>,

    #[pyo3(get)]
    pub entity_type: EntityType,
}

#[pymethods]
impl Features {
    #[new]
    /// Creates a new Features instance.
    /// A user may supply either a list of features or a single feature.
    /// Extract features into a Vec<Feature>
    /// Extraction follows the following rules:
    /// 1. Check if Pylist, if so, extract to Vec<Feature>
    /// 2. Check if PyDict, if so, iterate over each key-value pair and create a Feature
    /// 3. If neither, return an error
    /// # Arguments
    /// * `features` - A Python object that can be a list of Feature instances or
    ///               a dictionary of key-value pairs where keys are feature names
    /// # Returns
    /// * `Features` - A new Features instance containing the extracted features.
    pub fn new(features: Bound<'_, PyAny>) -> Result<Self, TypeError> {
        let features = if features.is_instance_of::<PyList>() {
            features
                .downcast::<PyList>()
                .unwrap()
                .iter()
                .map(|item| item.extract::<Feature>().unwrap())
                .collect()
        } else if features.is_instance_of::<PyDict>() {
            features
                .downcast::<PyDict>()
                .unwrap()
                .iter()
                .map(|(key, value)| {
                    Feature::new(&key.extract::<String>().unwrap(), value.clone()).unwrap()
                })
                .collect()
        } else {
            Err(TypeError::UnsupportedFeaturesTypeError(
                features.get_type().name()?.to_string(),
            ))?
        };
        Ok(Features {
            features,
            entity_type: EntityType::Feature,
        })
    }

    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

impl Features {
    pub fn iter(&self) -> std::slice::Iter<'_, Feature> {
        self.features.iter()
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct FeatureMap {
    #[pyo3(get)]
    pub features: HashMap<String, HashMap<String, usize>>,
}

#[pymethods]
impl FeatureMap {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }
}

#[pyclass]
#[derive(Clone, Serialize, Debug)]
pub struct Metric {
    pub name: String,
    pub value: f64,
}

#[pymethods]
impl Metric {
    #[new]
    pub fn new(name: String, value: Bound<'_, PyAny>) -> Self {
        let value = if value.is_instance_of::<PyFloat>() {
            value.extract::<f64>().unwrap()
        } else if value.is_instance_of::<PyInt>() {
            value.extract::<i64>().unwrap() as f64
        } else {
            panic!(
                "Unsupported metric type: {}",
                value.get_type().name().unwrap()
            );
        };
        let lowercase_name = name.to_lowercase();
        Metric {
            name: lowercase_name,
            value,
        }
    }

    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

impl Metric {
    pub fn new_rs(name: String, value: f64) -> Self {
        Metric { name, value }
    }
}

#[pyclass]
#[derive(Clone, Serialize, Debug)]
pub struct Metrics {
    #[pyo3(get)]
    pub metrics: Vec<Metric>,

    #[pyo3(get)]
    pub entity_type: EntityType,
}

#[pymethods]
impl Metrics {
    #[new]
    pub fn new(metrics: Bound<'_, PyAny>) -> Result<Self, TypeError> {
        let metrics = if metrics.is_instance_of::<PyList>() {
            metrics
                .downcast::<PyList>()
                .unwrap()
                .iter()
                .map(|item| item.extract::<Metric>().unwrap())
                .collect()
        } else if metrics.is_instance_of::<PyDict>() {
            metrics
                .downcast::<PyDict>()
                .unwrap()
                .iter()
                .map(|(key, value)| Metric::new(key.extract().unwrap(), value))
                .collect()
        } else {
            Err(TypeError::UnsupportedMetricsTypeError(
                metrics.get_type().name()?.to_string(),
            ))?
        };
        Ok(Metrics {
            metrics,
            entity_type: EntityType::Metric,
        })
    }
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

impl Metrics {
    pub fn iter(&self) -> std::slice::Iter<'_, Metric> {
        self.metrics.iter()
    }
}

#[derive(Clone, Serialize, Debug)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
pub struct LLMTaskRecord {
    pub uid: String,
    pub entity_id: i32,
    pub created_at: DateTime<Utc>,
    pub context: Value,
    pub score: Value,
    pub prompt: Option<Value>,
}

#[pyclass]
#[derive(Clone, Serialize, Debug)]
pub struct LLMRecord {
    pub uid: String,
    pub created_at: DateTime<Utc>,
    pub context: Value,
    pub score: Value,
    pub prompt: Option<Value>,
    #[pyo3(get)]
    pub entity_type: EntityType,
}

#[pymethods]
impl LLMRecord {
    #[new]
    #[pyo3(signature = (
        context,
        prompt=None,
    ))]

    /// Creates a new LLMRecord instance.
    /// The context is either a python dictionary or a pydantic basemodel.
    pub fn new(
        py: Python<'_>,
        context: Bound<'_, PyAny>,
        prompt: Option<Bound<'_, PyAny>>,
    ) -> Result<Self, TypeError> {
        // check if context is a PyDict or PyObject(Pydantic model)
        let context_val = if context.is_instance_of::<PyDict>() {
            pyobject_to_json(&context)?
        } else if is_pydantic_basemodel(py, &context)? {
            // Dump pydantic model to dictionary
            let model = context.call_method0("model_dump")?;

            // Serialize the dictionary to JSON
            pyobject_to_json(&model)?
        } else {
            Err(TypeError::MustBeDictOrBaseModel)?
        };

        let prompt: Option<Value> = match prompt {
            Some(p) => {
                if p.is_instance_of::<Prompt>() {
                    let prompt = p.extract::<Prompt>()?;
                    Some(serde_json::to_value(prompt)?)
                } else {
                    Some(pyobject_to_json(&p)?)
                }
            }
            None => None,
        };

        Ok(LLMRecord {
            uid: create_uuid7(),
            created_at: Utc::now(),
            context: context_val,
            score: Value::Null,
            prompt,
            entity_type: EntityType::LLM,
        })
    }

    #[getter]
    pub fn context<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyAny>, TypeError> {
        Ok(json_to_pyobject_value(py, &self.context)?
            .into_bound_py_any(py)?
            .clone())
    }
}

impl LLMRecord {
    pub fn new_rs(context: Option<Value>, prompt: Option<Value>) -> Self {
        LLMRecord {
            context: context.unwrap_or(Value::Object(serde_json::Map::new())),
            prompt,
            entity_type: EntityType::LLM,
            uid: create_uuid7(),
            created_at: Utc::now(),
            score: Value::Null,
        }
    }

    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

#[derive(Debug)]
pub enum QueueItem {
    Features(Features),
    Metrics(Metrics),
    LLM(Box<LLMRecord>),
}

impl QueueItem {
    /// Helper for extracting an Entity from a Python object
    pub fn from_py_entity(entity: &Bound<'_, PyAny>) -> Result<Self, TypeError> {
        let entity_type = entity.getattr("entity_type")?.extract::<EntityType>()?;

        match entity_type {
            EntityType::Feature => {
                let features = entity.extract::<Features>()?;
                Ok(QueueItem::Features(features))
            }
            EntityType::Metric => {
                let metrics = entity.extract::<Metrics>()?;
                Ok(QueueItem::Metrics(metrics))
            }
            EntityType::LLM => {
                // LLM is not supported in this context
                let llm = entity.extract::<LLMRecord>()?;
                Ok(QueueItem::LLM(Box::new(llm)))
            }
        }
    }
}

pub trait QueueExt: Send + Sync {
    fn metrics(&self) -> &Vec<Metric>;
    fn features(&self) -> &Vec<Feature>;
    fn llm_records(&self) -> Vec<&LLMRecord>;
}

impl QueueExt for Features {
    fn metrics(&self) -> &Vec<Metric> {
        // this is not a real implementation, just a placeholder
        // to satisfy the trait bound
        static EMPTY: Vec<Metric> = Vec::new();
        &EMPTY
    }

    fn features(&self) -> &Vec<Feature> {
        &self.features
    }

    fn llm_records(&self) -> Vec<&LLMRecord> {
        // this is not a real implementation, just a placeholder
        // to satisfy the trait bound
        vec![]
    }
}

impl QueueExt for Metrics {
    fn metrics(&self) -> &Vec<Metric> {
        &self.metrics
    }

    fn features(&self) -> &Vec<Feature> {
        // this is not a real implementation, just a placeholder
        // to satisfy the trait bound
        static EMPTY: Vec<Feature> = Vec::new();
        &EMPTY
    }

    fn llm_records(&self) -> Vec<&LLMRecord> {
        // this is not a real implementation, just a placeholder
        // to satisfy the trait bound
        vec![]
    }
}

impl QueueExt for LLMRecord {
    fn metrics(&self) -> &Vec<Metric> {
        // this is not a real implementation, just a placeholder
        // to satisfy the trait bound
        static EMPTY: Vec<Metric> = Vec::new();
        &EMPTY
    }

    fn features(&self) -> &Vec<Feature> {
        // this is not a real implementation, just a placeholder
        // to satisfy the trait bound
        static EMPTY: Vec<Feature> = Vec::new();
        &EMPTY
    }

    fn llm_records(&self) -> Vec<&LLMRecord> {
        vec![self]
    }
}
