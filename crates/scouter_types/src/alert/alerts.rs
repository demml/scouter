use crate::util::PyHelperFuncs;
use chrono::{DateTime, Utc};
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};

#[pyclass(eq)]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum AlertThreshold {
    Below,
    Above,
    Outside,
}

impl Display for AlertThreshold {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[pymethods]
impl AlertThreshold {
    #[staticmethod]
    pub fn from_value(value: &str) -> Option<Self> {
        match value.to_lowercase().as_str() {
            "below" => Some(AlertThreshold::Below),
            "above" => Some(AlertThreshold::Above),
            "outside" => Some(AlertThreshold::Outside),
            _ => None,
        }
    }

    pub fn __str__(&self) -> String {
        match self {
            AlertThreshold::Below => "Below".to_string(),
            AlertThreshold::Above => "Above".to_string(),
            AlertThreshold::Outside => "Outside".to_string(),
        }
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct AlertCondition {
    /// The reference value to compare against
    #[pyo3(get, set)]
    pub baseline_value: f64,

    #[pyo3(get, set)]
    pub alert_threshold: AlertThreshold,

    /// Optional delta value that modifies the baseline to create the alert boundary.
    /// The interpretation depends on alert_threshold:
    /// - Above: alert if value > (baseline + delta)
    /// - Below: alert if value < (baseline - delta)
    /// - Outside: alert if value is outside [baseline - delta, baseline + delta]
    #[pyo3(get, set)]
    pub delta: Option<f64>,
}

#[pymethods]
impl AlertCondition {
    #[new]
    #[pyo3(signature = (baseline_value, alert_threshold, delta=None))]
    pub fn new(baseline_value: f64, alert_threshold: AlertThreshold, delta: Option<f64>) -> Self {
        Self {
            baseline_value,
            alert_threshold,
            delta,
        }
    }

    /// Returns the upper bound for the alert condition
    pub fn upper_bound(&self) -> f64 {
        match self.delta {
            Some(d) => self.baseline_value + d,
            None => self.baseline_value,
        }
    }

    /// Returns the lower bound for the alert condition
    pub fn lower_bound(&self) -> f64 {
        match self.delta {
            Some(d) => self.baseline_value - d,
            None => self.baseline_value,
        }
    }

    /// Checks if a value should trigger an alert
    pub fn should_alert(&self, value: f64) -> bool {
        match (&self.alert_threshold, self.delta) {
            (AlertThreshold::Above, Some(d)) => value > (self.baseline_value + d),
            (AlertThreshold::Above, None) => value > self.baseline_value,
            (AlertThreshold::Below, Some(d)) => value < (self.baseline_value - d),
            (AlertThreshold::Below, None) => value < self.baseline_value,
            (AlertThreshold::Outside, Some(d)) => {
                value < (self.baseline_value - d) || value > (self.baseline_value + d)
            }
            (AlertThreshold::Outside, None) => value != self.baseline_value,
        }
    }

    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

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
