use crate::util::PyHelperFuncs;
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
    pub bins: BTreeMap<i32, f64>,
}

#[pymethods]
impl BinnedPsiMetric {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
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
        PyHelperFuncs::__str__(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FeatureBinProportionResult {
    pub feature: String,
    pub created_at: Vec<DateTime<Utc>>,
    pub bin_proportions: Vec<BTreeMap<i32, f64>>,
    pub overall_proportions: BTreeMap<i32, f64>,
}

#[cfg(feature = "server")]
impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for FeatureBinProportionResult {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;

        let bin_proportions_json: Vec<serde_json::Value> = row.try_get("bin_proportions")?;
        let bin_proportions: Vec<BTreeMap<i32, f64>> = bin_proportions_json
            .into_iter()
            .map(|json| serde_json::from_value(json).map_err(|e| sqlx::Error::Decode(Box::new(e))))
            .collect::<Result<Vec<_>, sqlx::Error>>()?;

        let overall_proportions_json: serde_json::Value = row.try_get("overall_proportions")?;
        let overall_proportions: BTreeMap<i32, f64> =
            serde_json::from_value(overall_proportions_json)
                .map_err(|e| sqlx::Error::Decode(Box::new(e)))?;

        Ok(FeatureBinProportionResult {
            feature: row.try_get("feature")?,
            created_at: row.try_get("created_at")?,
            bin_proportions,
            overall_proportions,
        })
    }
}
