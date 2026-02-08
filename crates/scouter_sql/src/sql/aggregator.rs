use crate::sql::error::SqlError;
use crate::sql::query::Queries;
use chrono::{DateTime, Duration, Timelike, Utc};
use dashmap::DashMap;
use scouter_types::{Attribute, TraceId, TraceSpanRecord, SCOUTER_ENTITY};
use sqlx::PgPool;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::OnceCell;
use tokio::time::{interval, Duration as StdDuration};
use tracing::{error, info, warn};

const TRACE_BATCH_SIZE: usize = 1000;

const MAX_TOTAL_SPANS: u64 = 1_000_000;

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
    pub entity_tags: HashSet<String>,
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

    /// Extracts specific entity attributes from span attributes and adds them to the aggregator's entity_tags set
    /// Arguments:
    /// - `span`: The TraceSpanRecord from which to extract entity attributes
    pub fn add_entities(&mut self, span: &TraceSpanRecord) {
        for attr in &span.attributes {
            if attr.key.starts_with(SCOUTER_ENTITY) {
                // Value should be string in the format "{uid}"
                let entity = match &attr.value {
                    serde_json::Value::String(s) => s.clone(),
                    _ => continue, // Skip if not a string
                };
                self.entity_tags.insert(entity);
            }
        }
    }

    pub fn new_from_span(span: &TraceSpanRecord) -> Self {
        let now = Utc::now();
        let mut aggregator = Self {
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
            entity_tags: HashSet::new(),
        };
        aggregator.add_entities(span);
        aggregator
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
        self.add_entities(span);
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
    traces: DashMap<TraceId, TraceAggregator>, // dashmap is rwlock internally
    pool: PgPool,
    max_traces: usize,
    total_span_count: AtomicU64,
}

impl TraceCache {
    fn new(pool: PgPool, max_traces: usize) -> Self {
        Self {
            traces: DashMap::new(),
            pool,
            max_traces,
            total_span_count: AtomicU64::new(0),
        }
    }

    /// Update trace aggregation from a span. Uses Arc<Self> to enable background flushing.
    pub async fn update_trace(self: &Arc<Self>, span: &TraceSpanRecord) {
        let current_traces = self.traces.len();
        let current_spans = self.total_span_count.load(Ordering::Relaxed);

        // Check trace and span pressure
        let trace_pressure = (current_traces * 100) / self.max_traces;
        let span_pressure = (current_spans * 100) / MAX_TOTAL_SPANS;
        let max_pressure = trace_pressure.max(span_pressure as usize);

        // If near capacity, trigger background flush
        if max_pressure >= 90 {
            warn!(
                current_traces,
                current_spans,
                max_pressure,
                "TraceCache high memory pressure, triggering background flush"
            );

            let cache = Arc::clone(self);
            tokio::spawn(async move {
                // Flush traces older than 5 seconds aggressively
                if let Err(e) = cache.flush_traces(Duration::seconds(5)).await {
                    error!(error = %e, "Background emergency flush failed");
                }
            });
        }
        self.traces
            .entry(span.trace_id.clone())
            .and_modify(|agg| {
                agg.update_from_span(span);
                self.total_span_count.fetch_add(1, Ordering::Relaxed);
            })
            .or_insert_with(|| {
                self.total_span_count.fetch_add(1, Ordering::Relaxed);
                TraceAggregator::new_from_span(span)
            });
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
        let mut spans_freed = 0u64;

        for id in stale_keys {
            if let Some((_, agg)) = self.traces.remove(&id) {
                spans_freed += agg.span_count as u64;
                to_flush.push((id, agg));
            }
        }

        // Decrement by actual span count, not trace count
        self.total_span_count
            .fetch_sub(spans_freed, Ordering::Relaxed);

        let count = to_flush.len();
        info!(
            flushed_traces = count,
            freed_spans = spans_freed,
            remaining_traces = self.traces.len(),
            remaining_spans = self.total_span_count.load(Ordering::Relaxed),
            "Flushed stale traces"
        );

        for chunk in to_flush.chunks(TRACE_BATCH_SIZE) {
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

        // Collect entity tags for batch insert
        let mut entity_trace_ids = Vec::new();
        let mut entity_uids = Vec::new();
        let mut entity_tagged_ats = Vec::new();
        let now = Utc::now();

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

            // Collect entity tags for this trace
            for entity_uid in &agg.entity_tags {
                entity_trace_ids.push(trace_id.as_bytes());
                entity_uids.push(entity_uid);
                entity_tagged_ats.push(now);
            }
        }

        // 1. Upsert trace aggregations
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

        // 2. Batch insert all entity tags in one query
        if !entity_trace_ids.is_empty() {
            sqlx::query(Queries::InsertTraceEntityTags.get_query())
                .bind(&entity_trace_ids)
                .bind(&entity_uids)
                .bind(&entity_tagged_ats)
                .execute(&self.pool)
                .await?;
        }

        Ok(())
    }
}

/// Initialize the global TraceCache singleton
pub async fn init_trace_cache(
    pool: PgPool,
    flush_interval: Duration,
    stale_threshold: Duration,
    max_traces: usize,
) -> Result<(), SqlError> {
    let cache = Arc::new(TraceCache::new(pool, max_traces));

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

            let current_traces = cache.traces.len();
            let current_spans = cache.total_span_count.load(Ordering::Relaxed);

            let threshold = if current_traces > max_traces || current_spans > MAX_TOTAL_SPANS {
                warn!(
                    current_traces,
                    current_spans, "Emergency flush triggered due to memory pressure"
                );
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
