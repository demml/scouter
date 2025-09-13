use crate::util::PyHelperFuncs;
use chrono::{DateTime, Utc};
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SpcDriftFeature {
    #[pyo3(get)]
    pub created_at: Vec<DateTime<Utc>>,

    #[pyo3(get)]
    pub values: Vec<f64>,
}

#[pymethods]
impl SpcDriftFeature {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }
}

#[pyclass(name = "BinnedSpcFeatureMetrics")]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SpcDriftFeatures {
    #[pyo3(get)]
    pub features: BTreeMap<String, SpcDriftFeature>,
}

#[pymethods]
impl SpcDriftFeatures {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }
}

impl SpcDriftFeatures {
    pub fn is_empty(&self) -> bool {
        self.features.is_empty()
    }
}
