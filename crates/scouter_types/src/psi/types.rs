use crate::util::ProfileFuncs;
use chrono::NaiveDateTime;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[pyclass]
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct BinnedPsiMetric {
    #[pyo3(get)]
    pub created_at: Vec<NaiveDateTime>,

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
