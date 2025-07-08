use crate::util::ProfileFuncs;
use chrono::{DateTime, Utc};
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BinnedPromptMetricStats {
    #[pyo3(get)]
    pub avg: f64,

    #[pyo3(get)]
    pub lower_bound: f64,

    #[pyo3(get)]
    pub upper_bound: f64,
}

#[pymethods]
impl BinnedPromptMetricStats {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BinnedPromptMetric {
    #[pyo3(get)]
    pub metric: String,

    #[pyo3(get)]
    pub created_at: Vec<DateTime<Utc>>,

    #[pyo3(get)]
    pub stats: Vec<BinnedPromptMetricStats>,
}

#[pymethods]
impl BinnedPromptMetric {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BinnedPromptMetrics {
    #[pyo3(get)]
    pub metrics: BTreeMap<String, BinnedPromptMetric>,
}

#[pymethods]
impl BinnedPromptMetrics {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}

impl BinnedPromptMetrics {
    pub fn from_vec(metrics: Vec<BinnedPromptMetric>) -> Self {
        let mapped: BTreeMap<String, BinnedPromptMetric> = metrics
            .into_iter()
            .map(|metric| (metric.metric.clone(), metric))
            .collect();
        BinnedPromptMetrics { metrics: mapped }
    }
}
