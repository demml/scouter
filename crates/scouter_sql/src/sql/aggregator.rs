use crate::sql::error::SqlError;
use crate::sql::query::Queries;
use chrono::{DateTime, Duration, Timelike, Utc};
use dashmap::DashMap;
use scouter_types::{Attribute, TraceId, TraceSpanRecord};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::OnceCell;
use tokio::time::{interval, Duration as StdDuration};
use tracing::{error, info, warn};

const DEFAULT_BATCH_SIZE: usize = 500;

// Global singleton instance using tokio's OnceCell for async-friendly init
static TRACE_CACHE: OnceCell<Arc<TraceCache>> = OnceCell::const_new();

#[derive(Debug, Clone)]
pub struct TraceAggregator {
    pub trace_id: TraceId,
    pub service_name: String,
    pub scope_name: String,
    pub scope_version: String,
    pub root_operation: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub status_code: i32,
    pub status_message: String,
    pub span_count: i64,
    pub error_count: i64,
    pub resource_attributes: Vec<Attribute>,
    pub first_seen: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
}

impl TraceAggregator {
    pub fn bucket_time(&self) -> DateTime<Utc> {
        self.start_time
            .with_minute(0)
            .unwrap()
            .with_second(0)
            .unwrap()
            .with_nanosecond(0)
            .unwrap()
    }

    pub fn new_from_span(span: &TraceSpanRecord) -> Self {
        let now = Utc::now();
        Self {
            trace_id: span.trace_id.clone(),
            service_name: span.service_name.clone(),
            scope_name: span.scope_name.clone(),
            scope_version: span.scope_version.clone().unwrap_or_default(),
            root_operation: if span.parent_span_id.is_none() {
                span.span_name.clone()
            } else {
                String::new()
            },
            start_time: span.start_time,
            end_time: Some(span.end_time),
            status_code: span.status_code,
            status_message: span.status_message.clone(),
            span_count: 1,
            error_count: if span.status_code == 2 { 1 } else { 0 },
            resource_attributes: span.resource_attributes.clone(),
            first_seen: now,
            last_updated: now,
        }
    }

    pub fn update_from_span(&mut self, span: &TraceSpanRecord) {
        if span.start_time < self.start_time {
            self.start_time = span.start_time;
        }
        if let Some(current_end) = self.end_time {
            if span.end_time > current_end {
                self.end_time = Some(span.end_time);
            }
        } else {
            self.end_time = Some(span.end_time);
        }

        if span.parent_span_id.is_none() {
            self.root_operation = span.span_name.clone();
            self.service_name = span.service_name.clone();
            self.scope_name = span.scope_name.clone();
            if let Some(version) = &span.scope_version {
                self.scope_version = version.clone();
            }
            self.resource_attributes = span.resource_attributes.clone();
        }

        if span.status_code == 2 {
            self.error_count += 1;
            self.status_code = 2;
            self.status_message = span.status_message.clone();
        }

        self.span_count += 1;
        self.last_updated = Utc::now();
    }

    pub fn duration_ms(&self) -> Option<i64> {
        self.end_time
            .map(|end| (end - self.start_time).num_milliseconds())
    }

    pub fn is_stale(&self, stale_duration: Duration) -> bool {
        (Utc::now() - self.last_updated) >= stale_duration
    }
}

pub struct TraceCache {
    traces: DashMap<TraceId, TraceAggregator>,
    pool: PgPool,
}

impl TraceCache {
    fn new(pool: PgPool) -> Self {
        Self {
            traces: DashMap::new(),
            pool,
        }
    }

    pub fn update_trace(&self, span: &TraceSpanRecord) {
        self.traces
            .entry(span.trace_id.clone())
            .and_modify(|agg| agg.update_from_span(span))
            .or_insert_with(|| TraceAggregator::new_from_span(span));
    }

    pub async fn flush_traces(&self, stale_threshold: Duration) -> Result<usize, SqlError> {
        let stale_keys: Vec<TraceId> = self
            .traces
            .iter()
            .filter(|entry| entry.value().is_stale(stale_threshold))
            .map(|entry| entry.key().clone())
            .collect();

        if stale_keys.is_empty() {
            return Ok(0);
        }

        let mut to_flush = Vec::with_capacity(stale_keys.len());
        for id in stale_keys {
            if let Some(pair) = self.traces.remove(&id) {
                to_flush.push(pair);
            }
        }

        let count = to_flush.len();
        for chunk in to_flush.chunks(DEFAULT_BATCH_SIZE) {
            self.upsert_batch(chunk).await?;
        }
        Ok(count)
    }

    async fn upsert_batch(&self, traces: &[(TraceId, TraceAggregator)]) -> Result<(), SqlError> {
        let mut created_ats = Vec::with_capacity(traces.len());
        let mut bucket_times = Vec::with_capacity(traces.len());
        let mut trace_ids = Vec::with_capacity(traces.len());
        let mut service_names = Vec::with_capacity(traces.len());
        let mut scope_names = Vec::with_capacity(traces.len());
        let mut scope_versions = Vec::with_capacity(traces.len());
        let mut root_operations = Vec::with_capacity(traces.len());
        let mut start_times = Vec::with_capacity(traces.len());
        let mut end_times = Vec::with_capacity(traces.len());
        let mut durations_ms = Vec::with_capacity(traces.len());
        let mut status_codes = Vec::with_capacity(traces.len());
        let mut status_messages = Vec::with_capacity(traces.len());
        let mut span_counts = Vec::with_capacity(traces.len());
        let mut error_counts = Vec::with_capacity(traces.len());
        let mut resource_attrs = Vec::with_capacity(traces.len());

        for (trace_id, agg) in traces {
            created_ats.push(agg.first_seen);
            bucket_times.push(agg.bucket_time());
            trace_ids.push(trace_id.as_bytes());
            service_names.push(&agg.service_name);
            scope_names.push(&agg.scope_name);
            scope_versions.push(&agg.scope_version);
            root_operations.push(&agg.root_operation);
            start_times.push(agg.start_time);
            end_times.push(agg.end_time.unwrap_or(agg.start_time));
            durations_ms.push(agg.duration_ms().unwrap_or(0));
            status_codes.push(agg.status_code);
            status_messages.push(&agg.status_message);
            span_counts.push(agg.span_count);
            error_counts.push(agg.error_count);
            resource_attrs.push(serde_json::to_value(&agg.resource_attributes)?);
        }

        sqlx::query(Queries::UpsertTrace.get_query())
            .bind(&bucket_times)
            .bind(&created_ats)
            .bind(&trace_ids)
            .bind(&service_names as &[&String])
            .bind(&scope_names as &[&String])
            .bind(&scope_versions as &[&String])
            .bind(&root_operations as &[&String])
            .bind(&start_times)
            .bind(&end_times)
            .bind(&durations_ms)
            .bind(&status_codes)
            .bind(&status_messages as &[&String])
            .bind(&span_counts)
            .bind(&error_counts)
            .bind(&resource_attrs)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

/// Initialize the global TraceCache singleton
pub async fn init_trace_cache(
    pool: PgPool,
    flush_interval: Duration,
    stale_threshold: Duration,
    max_cache_size: usize,
) -> Result<(), SqlError> {
    let cache = Arc::new(TraceCache::new(pool));

    if TRACE_CACHE.set(cache.clone()).is_err() {
        return Err(SqlError::TraceCacheError(
            "TraceCache singleton already initialized".to_string(),
        ));
    }

    let flush_std_duration = StdDuration::from_secs(flush_interval.num_seconds() as u64);

    // Spawn the background worker
    tokio::spawn(async move {
        let mut ticker = interval(flush_std_duration);
        loop {
            ticker.tick().await;

            let current_size = cache.traces.len();
            let threshold = if current_size > max_cache_size {
                warn!(current_size, "Emergency flush triggered");
                Duration::seconds(0)
            } else {
                stale_threshold
            };

            if let Err(e) = cache.flush_traces(threshold).await {
                error!(error = %e, "Flush task failed");
            }
        }
    });

    info!("TraceCache singleton initialized");
    Ok(())
}

/// Get access to the singleton
pub fn get_trace_cache() -> Arc<TraceCache> {
    TRACE_CACHE
        .get()
        .cloned()
        .expect("TraceCache not initialized")
}

/// Call this during shutdown to flush the final 15s of data
pub async fn shutdown_trace_cache() -> Result<usize, SqlError> {
    if let Some(cache) = TRACE_CACHE.get() {
        info!("Flushing TraceCache for shutdown...");
        cache.flush_traces(Duration::seconds(-1)).await // Force flush all by setting negative threshold
    } else {
        Ok(0)
    }
}
