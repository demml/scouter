use crate::util::PyHelperFuncs;
use chrono::{DateTime, Utc};
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[pyclass(from_py_object)]
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

#[pyclass(skip_from_py_object, name = "BinnedSpcFeatureMetrics")]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct SpcDriftFeatures {
    #[pyo3(get)]
    #[cfg_attr(feature = "utoipa", schema(value_type = serde_json::Value))]
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
