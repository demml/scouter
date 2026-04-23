use crate::sql::error::SqlError;
use crate::sql::query::Queries;
use crate::sql::utils::UuidBytea;
use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use scouter_dataframe::parquet::tracing::dispatch::{
    DispatchEventType, TraceDispatchRecord, TraceDispatchService,
};
use scouter_dataframe::parquet::tracing::summary::TraceSummaryService;
use scouter_types::{
    Attribute, TraceId, TraceSpanRecord, TraceSummaryRecord, SCOUTER_ENTITY, SCOUTER_QUEUE_RECORD,
};
use sqlx::PgPool;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration as StdDuration};
use tracing::{error, info, warn};
const TRACE_BATCH_SIZE: usize = 1000;

// ── Global TraceSummaryService singleton ─────────────────────────────────────
/// Uses `RwLock<Option<...>>` so tests can re-initialize with a fresh service.
static TRACE_SUMMARY_SERVICE: std::sync::RwLock<Option<Arc<TraceSummaryService>>> =
    std::sync::RwLock::new(None);
static TRACE_DISPATCH_SERVICE: std::sync::RwLock<Option<Arc<TraceDispatchService>>> =
    std::sync::RwLock::new(None);

/// Register the global TraceSummaryService. Replaces any previously registered instance.
pub fn init_trace_summary_service(service: Arc<TraceSummaryService>) -> Result<(), SqlError> {
    let mut guard = TRACE_SUMMARY_SERVICE
        .write()
        .map_err(|e| SqlError::TraceCacheError(format!("Failed to acquire write lock: {}", e)))?;
    *guard = Some(service);
    info!("TraceSummaryService global singleton registered in aggregator");
    Ok(())
}

/// Retrieve the global TraceSummaryService (if initialized).
pub fn get_trace_summary_service() -> Option<Arc<TraceSummaryService>> {
    TRACE_SUMMARY_SERVICE.read().ok()?.clone()
}

/// Register the global TraceDispatchService. Replaces any previously registered instance.
pub fn init_trace_dispatch_service(service: Arc<TraceDispatchService>) -> Result<(), SqlError> {
    let mut guard = TRACE_DISPATCH_SERVICE
        .write()
        .map_err(|e| SqlError::TraceCacheError(format!("Failed to acquire write lock: {}", e)))?;
    *guard = Some(service);
    info!("TraceDispatchService global singleton registered in aggregator");
    Ok(())
}

/// Retrieve the global TraceDispatchService (if initialized).
pub fn get_trace_dispatch_service() -> Option<Arc<TraceDispatchService>> {
    TRACE_DISPATCH_SERVICE.read().ok()?.clone()
}

const MAX_TOTAL_SPANS: u64 = 1_000_000;

/// Cache handle to manage trace aggregations
struct TraceCacheHandle {
    cache: Arc<TraceCache>,
    shutdown_flag: Arc<AtomicBool>,
}

static TRACE_CACHE: RwLock<Option<TraceCacheHandle>> = RwLock::const_new(None);

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
    pub entity_tags: HashSet<UuidBytea>,
    pub queue_tags: HashSet<UuidBytea>,
    pub queue_owned_entity_tags: HashSet<UuidBytea>,
}

fn extract_value_to_set(attr: &Attribute, set: &mut HashSet<UuidBytea>) -> Option<UuidBytea> {
    if let serde_json::Value::String(s) = &attr.value {
        match UuidBytea::from_uuid(s) {
            Ok(uid) => {
                set.insert(uid.clone());
                return Some(uid);
            }
            Err(e) => {
                warn!(%s, "Failed to parse value as UUID for attribute key '{}': {}", attr.key, e)
            }
        }
    }
    None
}

impl TraceAggregator {
    fn add_ids_from_attributes(&mut self, attributes: &[Attribute]) {
        let mut event_entity_tags = HashSet::new();
        let mut has_queue_tag = false;

        for attr in attributes {
            if attr.key == SCOUTER_QUEUE_RECORD {
                has_queue_tag |= extract_value_to_set(attr, &mut self.queue_tags).is_some();
            }
            if attr.key.starts_with(SCOUTER_ENTITY) {
                if let Some(entity_uid) = extract_value_to_set(attr, &mut self.entity_tags) {
                    event_entity_tags.insert(entity_uid);
                }
            }
        }

        if has_queue_tag {
            self.queue_owned_entity_tags.extend(event_entity_tags);
        }
    }

    /// Extracts specific attributes from span events to populate entity and queue tag sets
    pub fn add_ids(&mut self, span: &TraceSpanRecord) {
        for event in &span.events {
            self.add_ids_from_attributes(&event.attributes);
        }
        self.add_ids_from_attributes(&span.attributes);
    }

    pub fn synthetic_dispatch_entity_tags(&self) -> impl Iterator<Item = &UuidBytea> {
        self.entity_tags.difference(&self.queue_owned_entity_tags)
    }

    pub fn has_synthetic_dispatch_candidates(&self) -> bool {
        self.synthetic_dispatch_entity_tags().next().is_some()
    }

    pub fn to_dispatch_records(
        &self,
        trace_id: &TraceId,
        created_at: DateTime<Utc>,
    ) -> Vec<TraceDispatchRecord> {
        let mut records =
            Vec::with_capacity(self.queue_owned_entity_tags.len() + self.entity_tags.len());

        // Queue-owned entities are acknowledged for membership checks but should not
        // trigger synthetic trace-dispatch candidates.
        for entity_uid in &self.queue_owned_entity_tags {
            records.push(TraceDispatchRecord {
                trace_id: *trace_id.as_bytes(),
                entity_uid: *entity_uid.as_bytes(),
                start_time: self.start_time,
                event_type: DispatchEventType::Ack,
                created_at,
            });
        }

        for entity_uid in self.synthetic_dispatch_entity_tags() {
            records.push(TraceDispatchRecord {
                trace_id: *trace_id.as_bytes(),
                entity_uid: *entity_uid.as_bytes(),
                start_time: self.start_time,
                event_type: DispatchEventType::Candidate,
                created_at,
            });
        }

        records
    }

    pub fn new_from_span(span: &TraceSpanRecord) -> Self {
        let now = Utc::now();
        let mut aggregator = Self {
            trace_id: span.trace_id,
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
            queue_tags: HashSet::new(),
            queue_owned_entity_tags: HashSet::new(),
        };
        aggregator.add_ids(span);
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
        self.add_ids(span);
    }

    pub fn duration_ms(&self) -> Option<i64> {
        self.end_time
            .map(|end| (end - self.start_time).num_milliseconds())
    }

    pub fn is_stale(&self, stale_duration: Duration) -> bool {
        (Utc::now() - self.last_updated) >= stale_duration
    }

    /// Convert to the lightweight `TraceSummaryRecord` for Delta Lake writes.
    pub fn to_summary_record(&self) -> TraceSummaryRecord {
        let entity_ids: Vec<String> = self
            .entity_tags
            .iter()
            .map(|e| uuid::Uuid::from_bytes(e.0).to_string())
            .collect();
        let queue_ids: Vec<String> = self
            .queue_tags
            .iter()
            .map(|q| uuid::Uuid::from_bytes(q.0).to_string())
            .collect();
        TraceSummaryRecord {
            trace_id: self.trace_id,
            service_name: self.service_name.clone(),
            scope_name: self.scope_name.clone(),
            scope_version: self.scope_version.clone(),
            root_operation: self.root_operation.clone(),
            start_time: self.start_time,
            end_time: self.end_time,
            status_code: self.status_code,
            status_message: self.status_message.clone(),
            span_count: self.span_count,
            error_count: self.error_count,
            resource_attributes: self.resource_attributes.clone(),
            entity_ids,
            queue_ids,
        }
    }
}

pub struct TraceCache {
    traces: DashMap<TraceId, TraceAggregator>,
    max_traces: usize,
    total_span_count: AtomicU64,
}

impl TraceCache {
    fn new(max_traces: usize) -> Self {
        Self {
            traces: DashMap::new(),
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

        // If near capacity, log warning (background flush task will handle it)
        if max_pressure >= 90 {
            warn!(
                current_traces,
                current_spans,
                max_pressure,
                "TraceCache high memory pressure, will flush on next interval"
            );
        }
        self.traces
            .entry(span.trace_id)
            .and_modify(|agg| {
                agg.update_from_span(span);
                self.total_span_count.fetch_add(1, Ordering::Relaxed);
            })
            .or_insert_with(|| {
                self.total_span_count.fetch_add(1, Ordering::Relaxed);
                TraceAggregator::new_from_span(span)
            });
    }

    pub async fn flush_traces(
        &self,
        pool: &PgPool,
        stale_threshold: Duration,
    ) -> Result<usize, SqlError> {
        let stale_keys: Vec<TraceId> = self
            .traces
            .iter()
            .filter(|entry| entry.value().is_stale(stale_threshold))
            .map(|entry| *entry.key())
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
            self.upsert_batch(pool, chunk).await?;
        }
        Ok(count)
    }

    /// Write a batch of trace aggregations.
    ///
    /// Primary: Delta Lake via `TraceSummaryService` (span counts, timing, error rates).
    /// Secondary: Postgres for entity tag associations only (unchanged).
    async fn upsert_batch(
        &self,
        pool: &PgPool,
        traces: &[(TraceId, TraceAggregator)],
    ) -> Result<(), SqlError> {
        let now = Utc::now();

        // ── Delta Lake: write summary records ────────────────────────────────
        if let Some(summary_service) = get_trace_summary_service() {
            let records: Vec<TraceSummaryRecord> = traces
                .iter()
                .map(|(_, agg)| agg.to_summary_record())
                .collect();
            if let Err(e) = summary_service.write_summaries(records).await {
                error!("Failed to write trace summaries to Delta Lake: {:?}", e);
            }
        }

        // ── Delta Lake: append dispatch/index records for trace-eval flow ────
        if let Some(dispatch_service) = get_trace_dispatch_service() {
            let mut dispatch_records = Vec::new();
            for (trace_id, agg) in traces {
                dispatch_records.extend(agg.to_dispatch_records(trace_id, now));
            }

            if let Err(e) = dispatch_service.write_records(dispatch_records).await {
                error!(
                    "Failed to write trace dispatch records to Delta Lake: {:?}",
                    e
                );
            }
        }

        // ── Postgres: entity tag associations only ────────────────────────────
        let mut entity_trace_ids = Vec::new();
        let mut entity_uids = Vec::new();
        let mut entity_tagged_ats = Vec::new();

        for (trace_id, agg) in traces {
            for entity_uid in &agg.entity_tags {
                entity_trace_ids.push(trace_id.as_bytes());
                entity_uids.push(entity_uid.as_bytes());
                entity_tagged_ats.push(now);
            }
        }

        if !entity_trace_ids.is_empty() {
            sqlx::query(Queries::InsertTraceEntityTags.get_query())
                .bind(&entity_trace_ids)
                .bind(&entity_uids)
                .bind(&entity_tagged_ats)
                .execute(pool)
                .await?;
        }

        Ok(())
    }
}

/// Initialize the TraceCache, replacing any previous instance.
/// The old background flush task is signaled to stop and any remaining
/// traces are flushed with the NEW pool before the cache is swapped.
pub async fn init_trace_cache(
    pool: PgPool,
    flush_interval: Duration,
    stale_threshold: Duration,
    max_traces: usize,
) -> Result<(), SqlError> {
    // Shut down any existing cache first
    let old_cache = {
        let guard = TRACE_CACHE.read().await;
        guard.as_ref().map(|handle| {
            handle.shutdown_flag.store(true, Ordering::SeqCst);
            handle.cache.clone()
        })
    };

    // Flush outside so we dont hold the lock
    if let Some(cache) = old_cache {
        info!("Flushing previous TraceCache before re-initialization...");
        if let Err(e) = cache.flush_traces(&pool, Duration::seconds(-1)).await {
            error!(error = %e, "Failed to flush previous TraceCache");
        }
    }

    let cache = Arc::new(TraceCache::new(max_traces));
    let shutdown_flag = Arc::new(AtomicBool::new(false));

    {
        let mut guard = TRACE_CACHE.write().await;
        *guard = Some(TraceCacheHandle {
            cache: cache.clone(),
            shutdown_flag: shutdown_flag.clone(),
        });
    }

    let flush_std_duration = StdDuration::from_secs(flush_interval.num_seconds() as u64);
    let task_shutdown = shutdown_flag.clone();

    tokio::spawn(async move {
        let mut ticker = interval(flush_std_duration);
        loop {
            ticker.tick().await;

            if task_shutdown.load(Ordering::SeqCst) {
                info!("TraceCache background flush task shutting down");
                break;
            }

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

            if let Err(e) = cache.flush_traces(&pool, threshold).await {
                error!(error = %e, "Flush task failed");
            }
        }
    });

    info!("TraceCache initialized");
    Ok(())
}

/// Get access to the current TraceCache
pub async fn get_trace_cache() -> Arc<TraceCache> {
    TRACE_CACHE
        .read()
        .await
        .as_ref()
        .expect("TraceCache not initialized")
        .cache
        .clone()
}

/// Flush all remaining traces during shutdown
pub async fn shutdown_trace_cache(pool: &PgPool) -> Result<usize, SqlError> {
    let cache_to_flush = {
        let guard = TRACE_CACHE.read().await;
        guard.as_ref().map(|handle| {
            handle.shutdown_flag.store(true, Ordering::SeqCst);
            handle.cache.clone()
        })
    };

    if let Some(cache) = cache_to_flush {
        info!("Flushing TraceCache for shutdown...");
        cache.flush_traces(pool, Duration::seconds(-1)).await
    } else {
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::TraceAggregator;
    use chrono::Utc;
    use scouter_types::{Attribute, SpanEvent, TraceId, TraceSpanRecord};
    use serde_json::Value;

    fn make_attr(key: &str, value: &str) -> Attribute {
        Attribute {
            key: key.to_string(),
            value: Value::String(value.to_string()),
        }
    }

    fn make_event(attributes: Vec<Attribute>) -> SpanEvent {
        SpanEvent {
            timestamp: Utc::now(),
            name: "event".to_string(),
            attributes,
            dropped_attributes_count: 0,
        }
    }

    fn make_span(attributes: Vec<Attribute>, events: Vec<SpanEvent>) -> TraceSpanRecord {
        TraceSpanRecord {
            trace_id: TraceId::from_bytes([7; 16]),
            attributes,
            events,
            ..Default::default()
        }
    }

    #[test]
    fn synthetic_dispatch_candidates_exclude_queue_owned_entities() {
        let queue_owned_entity = "11111111-1111-1111-1111-111111111111";
        let trace_only_entity = "22222222-2222-2222-2222-222222222222";
        let queue_record_uid = "33333333-3333-3333-3333-333333333333";

        let span = make_span(
            Vec::new(),
            vec![
                make_event(vec![
                    make_attr("scouter.queue.record", queue_record_uid),
                    make_attr("scouter.entity", queue_owned_entity),
                ]),
                make_event(vec![make_attr(
                    &format!("scouter.entity.{}", trace_only_entity),
                    trace_only_entity,
                )]),
            ],
        );

        let agg = TraceAggregator::new_from_span(&span);
        let synthetic_entities: Vec<String> = agg
            .synthetic_dispatch_entity_tags()
            .map(|uid| uuid::Uuid::from_bytes(uid.0).to_string())
            .collect();

        assert!(agg
            .queue_tags
            .iter()
            .any(|uid| uuid::Uuid::from_bytes(uid.0).to_string() == queue_record_uid));
        assert_eq!(synthetic_entities, vec![trace_only_entity.to_string()]);
    }

    #[test]
    fn synthetic_dispatch_candidates_include_all_entities_when_no_queue_records_exist() {
        let first_entity = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
        let second_entity = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";
        let span = make_span(
            vec![make_attr(
                &format!("scouter.entity.{}", first_entity),
                first_entity,
            )],
            vec![make_event(vec![make_attr(
                &format!("scouter.entity.{}", second_entity),
                second_entity,
            )])],
        );

        let agg = TraceAggregator::new_from_span(&span);
        let synthetic_entities: std::collections::HashSet<String> = agg
            .synthetic_dispatch_entity_tags()
            .map(|uid| uuid::Uuid::from_bytes(uid.0).to_string())
            .collect();

        assert!(agg.queue_tags.is_empty());
        assert_eq!(
            synthetic_entities,
            std::collections::HashSet::from([first_entity.to_string(), second_entity.to_string(),])
        );
    }

    #[test]
    fn dispatch_records_include_ack_for_queue_owned_and_candidate_for_synthetic() {
        let queue_owned_entity = "11111111-1111-1111-1111-111111111111";
        let trace_only_entity = "22222222-2222-2222-2222-222222222222";
        let queue_record_uid = "33333333-3333-3333-3333-333333333333";

        let span = make_span(
            Vec::new(),
            vec![
                make_event(vec![
                    make_attr("scouter.queue.record", queue_record_uid),
                    make_attr("scouter.entity", queue_owned_entity),
                ]),
                make_event(vec![make_attr(
                    &format!("scouter.entity.{}", trace_only_entity),
                    trace_only_entity,
                )]),
            ],
        );

        let agg = TraceAggregator::new_from_span(&span);
        let now = Utc::now();
        let dispatch_records = agg.to_dispatch_records(&span.trace_id, now);

        let has_ack_queue_owned = dispatch_records.iter().any(|record| {
            record.event_type
                == scouter_dataframe::parquet::tracing::dispatch::DispatchEventType::Ack
                && uuid::Uuid::from_bytes(record.entity_uid).to_string() == queue_owned_entity
        });
        let has_candidate_trace_only = dispatch_records.iter().any(|record| {
            record.event_type
                == scouter_dataframe::parquet::tracing::dispatch::DispatchEventType::Candidate
                && uuid::Uuid::from_bytes(record.entity_uid).to_string() == trace_only_entity
        });

        assert!(has_ack_queue_owned);
        assert!(has_candidate_trace_only);
    }
}
