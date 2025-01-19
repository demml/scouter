use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BinnedCustomMetricStats {
    pub avg: f64,
    pub lower_bound: f64,
    pub upper_bound: f64,
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BinnedCustomMetric {
    pub metric: String,
    pub created_at: Vec<chrono::NaiveDateTime>,
    pub stats: Vec<BinnedCustomMetricStats>,
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BinnedCustomMetrics {
    pub metrics: BTreeMap<String, BinnedCustomMetric>,
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
