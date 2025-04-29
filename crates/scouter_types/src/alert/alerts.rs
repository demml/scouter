use crate::util::ProfileFuncs;
use chrono::{DateTime, Utc};
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    #[pyo3(get)]
    pub created_at: DateTime<Utc>,

    #[pyo3(get)]
    pub name: String,

    #[pyo3(get)]
    pub space: String,

    #[pyo3(get)]
    pub version: String,

    #[pyo3(get)]
    pub feature: String,

    #[pyo3(get)]
    pub alert: BTreeMap<String, String>,

    #[pyo3(get)]
    pub id: i32,

    #[pyo3(get)]
    pub drift_type: String,

    #[pyo3(get)]
    pub active: bool,
}

#[pymethods]
impl Alert {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Alerts {
    pub alerts: Vec<Alert>,
}
