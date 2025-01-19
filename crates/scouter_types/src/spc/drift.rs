use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpcDriftFeature {
    pub created_at: Vec<chrono::NaiveDateTime>,
    pub values: Vec<f64>,
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpcDriftFeatures {
    pub features: BTreeMap<String, SpcDriftFeature>,
}
