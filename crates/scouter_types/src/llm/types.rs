use crate::util::ProfileFuncs;
use chrono::{DateTime, Utc};
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BinnedLLMMetricStats {
    #[pyo3(get)]
    pub avg: f64,

    #[pyo3(get)]
    pub lower_bound: f64,

    #[pyo3(get)]
    pub upper_bound: f64,
}

#[pymethods]
impl BinnedLLMMetricStats {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BinnedLLMMetric {
    #[pyo3(get)]
    pub metric: String,

    #[pyo3(get)]
    pub created_at: Vec<DateTime<Utc>>,

    #[pyo3(get)]
    pub stats: Vec<BinnedLLMMetricStats>,
}

#[pymethods]
impl BinnedLLMMetric {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BinnedLLMMetrics {
    #[pyo3(get)]
    pub metrics: BTreeMap<String, BinnedLLMMetric>,
}

#[pymethods]
impl BinnedLLMMetrics {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}

impl BinnedLLMMetrics {
    pub fn from_vec(metrics: Vec<BinnedLLMMetric>) -> Self {
        let mapped: BTreeMap<String, BinnedLLMMetric> = metrics
            .into_iter()
            .map(|metric| (metric.metric.clone(), metric))
            .collect();
        BinnedLLMMetrics { metrics: mapped }
    }
}
