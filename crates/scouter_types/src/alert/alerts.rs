use crate::util::PyHelperFuncs;
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
    pub entity_name: String,

    #[pyo3(get)]
    pub alert: BTreeMap<String, String>,

    #[pyo3(get)]
    pub id: i32,

    #[pyo3(get)]
    pub active: bool,
}

#[cfg(feature = "server")]
impl<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> for Alert {
    fn from_row(row: &'r sqlx::postgres::PgRow) -> Result<Self, sqlx::Error> {
        use sqlx::Row;

        let alert_value: serde_json::Value = row.try_get("alert")?;
        let alert: BTreeMap<String, String> =
            serde_json::from_value(alert_value).unwrap_or_default();

        Ok(Alert {
            created_at: row.try_get("created_at")?,
            alert,
            entity_name: row.try_get("entity_name")?,
            id: row.try_get("id")?,
            active: row.try_get("active")?,
        })
    }
}

#[pymethods]
impl Alert {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Alerts {
    pub alerts: Vec<Alert>,
}
