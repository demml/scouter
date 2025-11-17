use crate::error::TypeError;
use crate::json_to_pyobject_value;
use crate::trace::{Attribute, SpanEvent, SpanLink};
use crate::PyHelperFuncs;
use chrono::{DateTime, Utc};
use pyo3::prelude::*;
use pyo3::IntoPyObjectExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "server")]
use sqlx::{postgres::PgRow, FromRow, Row};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
#[pyclass]
pub struct TraceListItem {
    #[pyo3(get)]
    pub trace_id: String,
    #[pyo3(get)]
    pub space: String,
    #[pyo3(get)]
    pub name: String,
    #[pyo3(get)]
    pub version: String,
    #[pyo3(get)]
    pub scope: String,
    #[pyo3(get)]
    pub service_name: Option<String>,
    #[pyo3(get)]
    pub root_operation: Option<String>,
    #[pyo3(get)]
    pub start_time: DateTime<Utc>,
    #[pyo3(get)]
    pub end_time: Option<DateTime<Utc>>,
    #[pyo3(get)]
    pub duration_ms: Option<i64>,
    #[pyo3(get)]
    pub status_code: i32,
    #[pyo3(get)]
    pub status_message: Option<String>,
    #[pyo3(get)]
    pub span_count: Option<i32>,
    #[pyo3(get)]
    pub has_errors: bool,
    #[pyo3(get)]
    pub error_count: i64,
    #[pyo3(get)]
    pub created_at: DateTime<Utc>,
}
#[pymethods]
impl TraceListItem {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[pyclass]
pub struct TraceSpan {
    #[pyo3(get)]
    pub trace_id: String,
    #[pyo3(get)]
    pub span_id: String,
    #[pyo3(get)]
    pub parent_span_id: Option<String>,
    #[pyo3(get)]
    pub span_name: String,
    #[pyo3(get)]
    pub span_kind: Option<String>,
    #[pyo3(get)]
    pub start_time: DateTime<Utc>,
    #[pyo3(get)]
    pub end_time: Option<DateTime<Utc>>,
    #[pyo3(get)]
    pub duration_ms: Option<i64>,
    #[pyo3(get)]
    pub status_code: i32,
    #[pyo3(get)]
    pub status_message: Option<String>,
    #[pyo3(get)]
    pub attributes: Vec<Attribute>,
    #[pyo3(get)]
    pub events: Vec<SpanEvent>,
    #[pyo3(get)]
    pub links: Vec<SpanLink>,
    #[pyo3(get)]
    pub depth: i32,
    #[pyo3(get)]
    pub path: Vec<String>,
    #[pyo3(get)]
    pub root_span_id: String,
    #[pyo3(get)]
    pub span_order: i32,
    pub input: Option<Value>,
    pub output: Option<Value>,
}

#[pymethods]
impl TraceSpan {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }

    #[getter]
    pub fn input<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyAny>, TypeError> {
        let input = match &self.input {
            Some(v) => v,
            None => &Value::Null,
        };
        Ok(json_to_pyobject_value(py, input)?
            .into_bound_py_any(py)?
            .clone())
    }

    #[getter]
    pub fn output<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyAny>, TypeError> {
        let output = match &self.output {
            Some(v) => v,
            None => &Value::Null,
        };
        Ok(json_to_pyobject_value(py, output)?
            .into_bound_py_any(py)?
            .clone())
    }
}

#[cfg(feature = "server")]
impl FromRow<'_, PgRow> for TraceSpan {
    fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        let attributes: Vec<Attribute> =
            serde_json::from_value(row.try_get("attributes")?).unwrap_or_default();
        let events: Vec<SpanEvent> =
            serde_json::from_value(row.try_get("events")?).unwrap_or_default();
        let links: Vec<SpanLink> =
            serde_json::from_value(row.try_get("links")?).unwrap_or_default();
        let input: Option<Value> = row.try_get("input")?;
        let output: Option<Value> = row.try_get("output")?;

        Ok(TraceSpan {
            trace_id: row.try_get("trace_id")?,
            span_id: row.try_get("span_id")?,
            parent_span_id: row.try_get("parent_span_id")?,
            span_name: row.try_get("span_name")?,
            span_kind: row.try_get("span_kind")?,
            start_time: row.try_get("start_time")?,
            end_time: row.try_get("end_time")?,
            duration_ms: row.try_get("duration_ms")?,
            status_code: row.try_get("status_code")?,
            status_message: row.try_get("status_message")?,
            attributes,
            events,
            links,
            depth: row.try_get("depth")?,
            path: row.try_get("path")?,
            root_span_id: row.try_get("root_span_id")?,
            span_order: row.try_get("span_order")?,
            input,
            output,
        })
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[pyclass]
pub struct TraceFilters {
    #[pyo3(get, set)]
    pub space: Option<String>,
    #[pyo3(get, set)]
    pub name: Option<String>,
    #[pyo3(get, set)]
    pub version: Option<String>,
    #[pyo3(get, set)]
    pub service_name: Option<String>,
    #[pyo3(get, set)]
    pub has_errors: Option<bool>,
    #[pyo3(get, set)]
    pub status_code: Option<i32>,
    #[pyo3(get, set)]
    pub start_time: Option<DateTime<Utc>>,
    #[pyo3(get, set)]
    pub end_time: Option<DateTime<Utc>>,
    #[pyo3(get, set)]
    pub limit: Option<i32>,
    #[pyo3(get, set)]
    pub cursor_created_at: Option<DateTime<Utc>>,
    #[pyo3(get, set)]
    pub cursor_trace_id: Option<String>,
}

#[pymethods]
#[allow(clippy::too_many_arguments)]
impl TraceFilters {
    #[new]
    #[pyo3(signature = (
        space=None,
        name=None,
        version=None,
        service_name=None,
        has_errors=None,
        status_code=None,
        start_time=None,
        end_time=None,
        limit=None,
        cursor_created_at=None,
        cursor_trace_id=None,
    ))]
    pub fn new(
        space: Option<String>,
        name: Option<String>,
        version: Option<String>,
        service_name: Option<String>,
        has_errors: Option<bool>,
        status_code: Option<i32>,
        start_time: Option<DateTime<Utc>>,
        end_time: Option<DateTime<Utc>>,
        limit: Option<i32>,
        cursor_created_at: Option<DateTime<Utc>>,
        cursor_trace_id: Option<String>,
    ) -> Self {
        TraceFilters {
            space,
            name,
            version,
            service_name,
            has_errors,
            status_code,
            start_time,
            end_time,
            limit,
            cursor_created_at,
            cursor_trace_id,
        }
    }
}

impl TraceFilters {
    pub fn with_space(mut self, space: impl Into<String>) -> Self {
        self.space = Some(space.into());
        self
    }

    pub fn with_service(mut self, service: impl Into<String>) -> Self {
        self.service_name = Some(service.into());
        self
    }

    pub fn with_errors_only(mut self) -> Self {
        self.has_errors = Some(true);
        self
    }

    pub fn with_time_range(mut self, start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        self.start_time = Some(start);
        self.end_time = Some(end);
        self
    }

    pub fn with_cursor(mut self, created_at: DateTime<Utc>, trace_id: impl Into<String>) -> Self {
        self.cursor_created_at = Some(created_at);
        self.cursor_trace_id = Some(trace_id.into());
        self
    }

    pub fn limit(mut self, limit: i32) -> Self {
        self.limit = Some(limit);
        self
    }
}

#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[pyclass]
pub struct TraceMetricBucket {
    #[pyo3(get)]
    pub bucket_start: DateTime<Utc>,
    #[pyo3(get)]
    pub trace_count: i64,
    #[pyo3(get)]
    pub avg_duration_ms: f64,
    #[pyo3(get)]
    pub p50_duration_ms: Option<f64>,
    #[pyo3(get)]
    pub p95_duration_ms: Option<f64>,
    #[pyo3(get)]
    pub p99_duration_ms: Option<f64>,
    #[pyo3(get)]
    pub error_rate: f64,
}

#[pymethods]
impl TraceMetricBucket {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}
