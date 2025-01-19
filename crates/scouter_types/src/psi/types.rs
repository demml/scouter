use chrono::NaiveDateTime;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[pyclass]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BinnedPsiMetric {
    pub created_at: Vec<NaiveDateTime>,
    pub psi: Vec<f64>,
    pub overall_psi: f64,
    pub bins: BTreeMap<usize, f64>,
}

#[pyclass]
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct BinnedPsiFeatureMetrics {
    pub features: BTreeMap<String, BinnedPsiMetric>,
}
