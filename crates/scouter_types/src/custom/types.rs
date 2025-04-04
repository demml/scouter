use crate::util::ProfileFuncs;
use chrono::{DateTime, Utc};
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BinnedCustomMetricStats {
    #[pyo3(get)]
    pub avg: f64,

    #[pyo3(get)]
    pub lower_bound: f64,

    #[pyo3(get)]
    pub upper_bound: f64,
}

#[pymethods]
impl BinnedCustomMetricStats {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BinnedCustomMetric {
    #[pyo3(get)]
    pub metric: String,

    #[pyo3(get)]
    pub created_at: Vec<DateTime<Utc>>,

    #[pyo3(get)]
    pub stats: Vec<BinnedCustomMetricStats>,
}

#[pymethods]
impl BinnedCustomMetric {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BinnedCustomMetrics {
    #[pyo3(get)]
    pub metrics: BTreeMap<String, BinnedCustomMetric>,
}

#[pymethods]
impl BinnedCustomMetrics {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}

impl BinnedCustomMetrics {
    pub fn from_vec(metrics: Vec<BinnedCustomMetric>) -> Self {
        let mapped: BTreeMap<String, BinnedCustomMetric> = metrics
            .into_iter()
            .map(|metric| (metric.metric.clone(), metric))
            .collect();
        BinnedCustomMetrics { metrics: mapped }
    }
}
