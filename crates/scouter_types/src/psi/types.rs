use crate::util::ProfileFuncs;
use chrono::{DateTime, Utc};
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[pyclass]
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct BinnedPsiMetric {
    #[pyo3(get)]
    pub created_at: Vec<DateTime<Utc>>,

    #[pyo3(get)]
    pub psi: Vec<f64>,

    #[pyo3(get)]
    pub overall_psi: f64,

    #[pyo3(get)]
    pub bins: BTreeMap<usize, f64>,
}

#[pymethods]
impl BinnedPsiMetric {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}

#[pyclass]
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct BinnedPsiFeatureMetrics {
    #[pyo3(get)]
    pub features: BTreeMap<String, BinnedPsiMetric>,
}

#[pymethods]
impl BinnedPsiFeatureMetrics {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureBinProportionResult {
    pub feature: String,
    pub created_at: Vec<DateTime<Utc>>,
    pub bin_proportions: Vec<BTreeMap<usize, f64>>,
    pub overall_proportions: BTreeMap<usize, f64>,
}

impl Default for FeatureBinProportionResult {
    fn default() -> Self {
        FeatureBinProportionResult {
            feature: String::new(),
            created_at: Vec::new(),
            bin_proportions: Vec::new(),
            overall_proportions: BTreeMap::new(),
        }
    }
}
