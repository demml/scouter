use crate::error::TypeError;
use crate::ProfileFuncs;
use pyo3::prelude::*;
use pyo3::types::{PyFloat, PyInt, PyString};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;

#[pyclass]
#[derive(Clone, Debug, Serialize)]
pub enum EntityType {
    Feature,
    Metric,
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
        ProfileFuncs::__str__(self)
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
        ProfileFuncs::__str__(self)
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
        ProfileFuncs::__str__(self)
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

#[pyclass]
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
        ProfileFuncs::__str__(self)
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
    pub fn new(features: Vec<Feature>) -> Self {
        Features {
            features,
            entity_type: EntityType::Feature,
        }
    }

    pub fn __str__(&self) -> String {
        ProfileFuncs::__str__(self)
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
        ProfileFuncs::__str__(self)
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
    pub fn new(name: String, value: f64) -> Self {
        Metric { name, value }
    }
    pub fn __str__(&self) -> String {
        ProfileFuncs::__str__(self)
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
    pub fn new(metrics: Vec<Metric>) -> Self {
        Metrics {
            metrics,
            entity_type: EntityType::Metric,
        }
    }
    pub fn __str__(&self) -> String {
        ProfileFuncs::__str__(self)
    }
}

impl Metrics {
    pub fn iter(&self) -> std::slice::Iter<'_, Metric> {
        self.metrics.iter()
    }
}

#[derive(Debug)]
pub enum QueueItem {
    Features(Features),
    Metrics(Metrics),
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
        }
    }
}

pub trait QueueExt: Send + Sync {
    fn metrics(&self) -> &Vec<Metric>;
    fn features(&self) -> &Vec<Feature>;
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
}
