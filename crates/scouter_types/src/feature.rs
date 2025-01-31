use crate::ProfileFuncs;
use pyo3::prelude::*;
use scouter_error::ScouterError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;

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
    pub fn to_float(&self, feature_map: &FeatureMap) -> Result<f64, ScouterError> {
        feature_map
            .features
            .get(&self.name)
            .and_then(|feat_map| {
                feat_map
                    .get(&self.value)
                    .or_else(|| feat_map.get("missing"))
            })
            .map(|&val| val as f64)
            .ok_or(ScouterError::MissingValue)
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

    pub fn __str__(&self) -> String {
        ProfileFuncs::__str__(self)
    }
}

impl Feature {
    pub fn to_float(&self, feature_map: &FeatureMap) -> Result<f64, ScouterError> {
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
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Features {
    pub features: Vec<Feature>,
}

#[pymethods]
impl Features {
    #[new]
    pub fn new(features: Vec<Feature>) -> Self {
        Features { features }
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
#[derive(Clone, Serialize, Deserialize, Debug)]
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
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Metrics {
    pub metrics: Vec<Metric>,
}

#[pymethods]
impl Metrics {
    #[new]
    pub fn new(metrics: Vec<Metric>) -> Self {
        Metrics { metrics }
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
