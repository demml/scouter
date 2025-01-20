use crate::util::ProfileFuncs;
use chrono::NaiveDateTime;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    #[pyo3(get)]
    pub created_at: NaiveDateTime,

    #[pyo3(get)]
    pub name: String,

    #[pyo3(get)]
    pub repository: String,

    #[pyo3(get)]
    pub version: String,

    #[pyo3(get)]
    pub feature: String,

    #[pyo3(get)]
    pub alert: BTreeMap<String, String>,

    #[pyo3(get)]
    pub id: i32,

    #[pyo3(get)]
    pub status: String,
}

#[pymethods]
impl Alert {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        ProfileFuncs::__str__(self)
    }
}
