use crate::trace::{Attribute, SpanEvent, SpanLink};
use chrono::{DateTime, Utc};
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
#[cfg(feature = "server")]
use sqlx::{postgres::PgRow, FromRow, Row};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "server", derive(sqlx::FromRow))]
#[pyclass]
pub struct TraceListItem {
    pub trace_id: String,
    pub space: String,
    pub name: String,
    pub version: String,
    pub scope: String,
    pub service_name: Option<String>,
    pub root_operation: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,
    pub status_code: i32,
    pub status_message: Option<String>,
    pub span_count: Option<i32>,
    pub has_errors: bool,
    pub error_count: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[pyclass]
pub struct TraceSpan {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub span_name: String,
    pub span_kind: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,
    pub status_code: String,
    pub status_message: Option<String>,
    pub attributes: Vec<Attribute>,
    pub events: Vec<SpanEvent>,
    pub links: Vec<SpanLink>,
    pub depth: i32,
    pub path: Vec<String>,
    pub root_span_id: String,
    pub span_order: i32,
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
        })
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[pyclass]
pub struct TraceFilters {
    pub space: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub service_name: Option<String>,
    pub has_errors: Option<bool>,
    pub status_code: Option<i32>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub limit: Option<i32>,
    pub cursor_created_at: Option<DateTime<Utc>>,
    pub cursor_trace_id: Option<String>,
}

impl TraceFilters {
    pub fn new() -> Self {
        Self::default()
    }

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
    pub bucket_start: DateTime<Utc>,
    pub trace_count: i64,
    pub avg_duration_ms: f64,
    pub p50_duration_ms: Option<f64>,
    pub p95_duration_ms: Option<f64>,
    pub p99_duration_ms: Option<f64>,
    pub error_rate: f64,
}
