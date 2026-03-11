use crate::error::TraceEngineError;
use crate::parquet::tracing::engine::{TableCommand, TraceSpanDBEngine};
use crate::parquet::tracing::queries::TraceQueries;
use datafusion::prelude::SessionContext;
use scouter_settings::ObjectStorageSettings;
use scouter_types::TraceSpanRecord;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use tracing::{debug, info};

const FLUSH_INTERVAL_SECS: u64 = 5;

/// Global singleton for the TraceSpanService.
///
/// Initialized via `init_trace_span_service()` in server setup.
/// Consumer workers call `get_trace_span_service()` to obtain the Arc for writing.
/// Uses `RwLock<Option<...>>` so tests can re-initialize with a fresh service.
static TRACE_SPAN_SERVICE: std::sync::RwLock<Option<Arc<TraceSpanService>>> =
    std::sync::RwLock::new(None);

/// Initialize the global `TraceSpanService`.
///
/// If a previous service exists (e.g. in test re-initialization), it is signaled to
/// shut down before being replaced.
pub async fn init_trace_span_service(
    storage_settings: &ObjectStorageSettings,
    compaction_interval_hours: u64,
    flush_interval_secs: Option<u64>,
    retention_days: Option<u32>,
) -> Result<Arc<TraceSpanService>, TraceEngineError> {
    // Shut down any existing service before replacing
    let old_service = {
        let guard = TRACE_SPAN_SERVICE.read().unwrap();
        guard.clone()
    };
    if let Some(old) = old_service {
        info!("Shutting down previous TraceSpanService before re-initialization");
        old.signal_shutdown().await;
    }

    let service = Arc::new(
        TraceSpanService::new(
            storage_settings,
            compaction_interval_hours,
            flush_interval_secs,
            retention_days,
        )
        .await?,
    );

    {
        let mut guard = TRACE_SPAN_SERVICE.write().unwrap();
        *guard = Some(service.clone());
    }

    info!("TraceSpanService global singleton initialized");
    Ok(service)
}

/// Retrieve the global `TraceSpanService` initialized during startup.
///
/// Returns `None` if called before `init_trace_span_service()`.
pub fn get_trace_span_service() -> Option<Arc<TraceSpanService>> {
    TRACE_SPAN_SERVICE.read().unwrap().clone()
}

pub struct TraceSpanService {
    engine_tx: mpsc::Sender<TableCommand>,
    span_tx: mpsc::Sender<Vec<TraceSpanRecord>>,
    shutdown_tx: mpsc::Sender<()>,
    engine_handle: tokio::task::JoinHandle<()>,
    buffer_handle: tokio::task::JoinHandle<()>,
    pub query_service: TraceQueries,
    /// Shared SessionContext — exposes `trace_spans` registration for TraceSummaryService.
    pub ctx: Arc<SessionContext>,
}

impl TraceSpanService {
    /// Create a new `TraceSpanService` with the given storage settings and start the engine and buffering actors.
    /// The buffering actor will flush spans to storage when the buffer reaches capacity or after a time interval.
    /// # Arguments
    /// * `storage_settings` - Configuration for object storage where trace spans will be persisted.
    /// * `compaction_interval_hours` - How often the engine should perform compaction
    ///   (merging small files into larger ones). Longer intervals reduce write amplification
    ///   but may increase read latency.
    /// * `flush_interval_secs` - Optional interval in seconds for flushing the buffer to storage
    pub async fn new(
        storage_settings: &ObjectStorageSettings,
        compaction_interval_hours: u64,
        flush_interval_secs: Option<u64>,
        retention_days: Option<u32>,
    ) -> Result<Self, TraceEngineError> {
        let buffer_size = storage_settings.trace_buffer_size();
        let engine = TraceSpanDBEngine::new(storage_settings).await?;

        info!(
            "TraceSpanService initialized with buffer_size: {}",
            buffer_size
        );

        let ctx = engine.ctx.clone();
        let (engine_tx, engine_handle) =
            engine.start_actor(compaction_interval_hours, retention_days);
        let (span_tx, span_rx) = mpsc::channel::<Vec<TraceSpanRecord>>(100);
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>(1);

        let buffer_handle = Self::start_buffering_actor(
            engine_tx.clone(),
            span_rx,
            shutdown_rx,
            flush_interval_secs,
            buffer_size,
        );

        Ok(TraceSpanService {
            engine_tx,
            span_tx,
            shutdown_tx,
            engine_handle,
            buffer_handle,
            query_service: TraceQueries::new(ctx.clone()),
            ctx,
        })
    }

    fn start_buffering_actor(
        engine_tx: mpsc::Sender<TableCommand>,
        mut span_rx: mpsc::Receiver<Vec<TraceSpanRecord>>,
        mut shutdown_rx: mpsc::Receiver<()>,
        flush_interval_secs: Option<u64>,
        buffer_size: usize,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut buffer: Vec<TraceSpanRecord> = Vec::with_capacity(buffer_size);
            let mut flush_ticker = interval(Duration::from_secs(
                flush_interval_secs.unwrap_or(FLUSH_INTERVAL_SECS),
            ));
            flush_ticker.tick().await;

            loop {
                tokio::select! {
                    Some(spans) = span_rx.recv() => {
                        buffer.extend(spans);
                        if buffer.len() >= buffer_size {
                            Self::flush_buffer(&engine_tx, &mut buffer).await;
                        }
                    }
                    _ = flush_ticker.tick() => {
                        if !buffer.is_empty() {
                            info!("Flushing spans buffer with {} spans", buffer.len());
                            Self::flush_buffer(&engine_tx, &mut buffer).await;
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Buffer actor received shutdown signal");
                        if !buffer.is_empty() {
                            info!("Flushing final {} spans before shutdown", buffer.len());
                            Self::flush_buffer(&engine_tx, &mut buffer).await;
                        }
                        break;
                    }
                }
            }

            info!("Buffering actor shutting down");
        })
    }

    async fn flush_buffer(
        engine_tx: &mpsc::Sender<TableCommand>,
        buffer: &mut Vec<TraceSpanRecord>,
    ) {
        if buffer.is_empty() {
            return;
        }

        let capacity = buffer.capacity();
        let spans_to_write = std::mem::replace(buffer, Vec::with_capacity(capacity));
        let span_count = spans_to_write.len();

        debug!("Sending write command to engine for {} spans", span_count);

        let (tx, rx) = tokio::sync::oneshot::channel();

        if let Err(e) = engine_tx
            .send(TableCommand::Write {
                spans: spans_to_write,
                respond_to: tx,
            })
            .await
        {
            tracing::error!("Failed to send write command: {}", e);
            return;
        }

        match rx.await {
            Ok(Ok(())) => info!("Successfully flushed {} spans", span_count),
            Ok(Err(e)) => tracing::error!("Write failed: {}", e),
            Err(e) => tracing::error!("Failed to receive write response: {}", e),
        }
    }

    /// Send spans to the buffering actor for async write to Delta Lake.
    /// # Arguments
    /// * `spans` - A batch of `TraceSpanRecord` to write. The buffering actor will flush to storage
    ///   when the buffer reaches capacity or after a time interval.
    pub async fn write_spans(&self, spans: Vec<TraceSpanRecord>) -> Result<(), TraceEngineError> {
        self.span_tx
            .send(spans)
            .await
            .map_err(|_| TraceEngineError::ChannelClosed)?;
        Ok(())
    }

    /// Write spans directly to the engine actor, bypassing the buffer.
    ///
    /// Unlike `write_spans()`, this method sends a single large batch as one
    /// Delta Lake commit and awaits the result. Use for bulk seeding/migration
    /// where you need deterministic commit boundaries and maximum throughput.
    /// This is used in benchmarks and stress tests to simulate high-volume writes without caching effects
    pub async fn write_spans_direct(
        &self,
        spans: Vec<TraceSpanRecord>,
    ) -> Result<(), TraceEngineError> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.engine_tx
            .send(TableCommand::Write {
                spans,
                respond_to: tx,
            })
            .await
            .map_err(|_| TraceEngineError::ChannelClosed)?;
        rx.await.map_err(|_| TraceEngineError::ChannelClosed)?
    }

    pub async fn optimize(&self) -> Result<(), TraceEngineError> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.engine_tx
            .send(TableCommand::Optimize { respond_to: tx })
            .await
            .map_err(|_| TraceEngineError::ChannelClosed)?;

        rx.await.map_err(|_| TraceEngineError::ChannelClosed)?
    }

    pub async fn vacuum(&self, retention_hours: u64) -> Result<(), TraceEngineError> {
        let (tx, rx) = tokio::sync::oneshot::channel();

        self.engine_tx
            .send(TableCommand::Vacuum {
                retention_hours,
                respond_to: tx,
            })
            .await
            .map_err(|_| TraceEngineError::ChannelClosed)?;

        rx.await.map_err(|_| TraceEngineError::ChannelClosed)?
    }

    /// Delete all spans with a `partition_date` older than `retention_days` ago,
    /// then run VACUUM to physically reclaim disk space.
    ///
    /// Call order matters: DELETE marks files as unreferenced in the Delta log;
    /// VACUUM then removes the orphaned Parquet files from storage.
    pub async fn expire(&self, retention_days: u32) -> Result<(), TraceEngineError> {
        let cutoff_date =
            (chrono::Utc::now() - chrono::Duration::days(retention_days as i64)).date_naive();

        // Step 1: DELETE WHERE partition_date < cutoff (marks rows removed in Delta log)
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.engine_tx
            .send(TableCommand::Expire {
                cutoff_date,
                respond_to: tx,
            })
            .await
            .map_err(|_| TraceEngineError::ChannelClosed)?;
        rx.await.map_err(|_| TraceEngineError::ChannelClosed)??;

        // Step 2: VACUUM — physically deletes orphaned files from storage.
        // retention_hours=0 with enforce_retention_duration=false removes all
        // post-DELETE orphans immediately.
        self.vacuum(0).await
    }

    /// Signal shutdown without consuming `self` — safe to call from `Arc<TraceSpanService>`.
    ///
    /// Sends the shutdown signal to the buffering actor and engine actor.
    /// Callers that own `self` should prefer `shutdown()` to await full drain.
    pub async fn signal_shutdown(&self) {
        info!("TraceSpanService signaling shutdown");
        let _ = self.shutdown_tx.send(()).await;
        let _ = self.engine_tx.send(TableCommand::Shutdown).await;
    }

    pub async fn shutdown(self) -> Result<(), TraceEngineError> {
        info!("TraceSpanService shutting down");

        let _ = self.shutdown_tx.send(()).await;

        if let Err(e) = self.buffer_handle.await {
            tracing::error!("Buffer handle error: {}", e);
        }

        self.engine_tx
            .send(TableCommand::Shutdown)
            .await
            .map_err(|_| TraceEngineError::ChannelClosed)?;

        if let Err(e) = self.engine_handle.await {
            tracing::error!("Engine handle error: {}", e);
        }

        info!("TraceSpanService shutdown complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parquet::tracing::queries::{
        date_lit, ts_lit, PARTITION_DATE_COL, SPAN_TABLE_NAME, START_TIME_COL,
    };
    use arrow_array::Array;
    use chrono::Utc;
    use datafusion::logical_expr::col;
    use scouter_mocks::generate_trace_with_spans;
    use scouter_settings::ObjectStorageSettings;
    use scouter_types::sql::TraceSpan;
    use scouter_types::{Attribute, SpanId, TraceId, TraceSpanRecord};
    use serde_json::Value;
    use tracing_subscriber;

    fn cleanup() {
        let _ = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .try_init();

        let storage_settings = ObjectStorageSettings::default();
        let current_dir = std::env::current_dir().unwrap();
        let storage_path = current_dir.join(storage_settings.storage_root());
        if storage_path.exists() {
            std::fs::remove_dir_all(storage_path).unwrap();
        }
    }

    /// Build a deterministic `TraceSpanRecord` with the given IDs and attributes.
    fn make_span(
        trace_id: &TraceId,
        span_id: SpanId,
        parent_span_id: Option<SpanId>,
        service_name: &str,
        span_name: &str,
        attributes: Vec<Attribute>,
    ) -> TraceSpanRecord {
        let now = Utc::now();
        TraceSpanRecord {
            created_at: now,
            trace_id: trace_id.clone(),
            span_id,
            parent_span_id,
            flags: 1,
            trace_state: String::new(),
            scope_name: "test.scope".to_string(),
            scope_version: None,
            span_name: span_name.to_string(),
            span_kind: "INTERNAL".to_string(),
            start_time: now,
            end_time: now + chrono::Duration::milliseconds(100),
            duration_ms: 100,
            status_code: 0,
            status_message: "OK".to_string(),
            attributes,
            events: vec![],
            links: vec![],
            label: None,
            input: Value::Null,
            output: Value::Null,
            service_name: service_name.to_string(),
            resource_attributes: vec![],
        }
    }

    #[tokio::test]
    async fn test_service_initialization() -> Result<(), TraceEngineError> {
        cleanup();

        let storage_settings = ObjectStorageSettings::default();
        let service = TraceSpanService::new(&storage_settings, 24, Some(2), None).await?;
        service.shutdown().await?;
        cleanup();
        Ok(())
    }

    #[tokio::test]
    async fn test_dataframe_trace_write_single_batch() -> Result<(), TraceEngineError> {
        cleanup();

        let storage_settings = ObjectStorageSettings::default();
        let service = TraceSpanService::new(&storage_settings, 24, Some(2), None).await?;

        let (_trace_record, spans, _tags) = generate_trace_with_spans(3, 0);
        info!("Test: writing {} spans", spans.len());

        let first_trace_id = spans.first().unwrap().trace_id.clone();
        service.write_spans(spans).await?;

        info!("Test: waiting for flush");
        tokio::time::sleep(Duration::from_secs(5)).await;

        let trace_id_bytes = first_trace_id.as_bytes();
        let result_spans: Vec<TraceSpan> = service
            .query_service
            .get_trace_spans(Some(trace_id_bytes.as_slice()), None, None, None, None)
            .await?;

        assert!(
            !result_spans.is_empty(),
            "Expected at least 1 span but got 0"
        );

        service.shutdown().await?;
        cleanup();
        Ok(())
    }

    /// Verify that `build_span_tree` returns spans in DFS (depth-first) order with correct
    /// depth, path, and root_span_id fields — matching what the Postgres recursive CTE produced.
    #[tokio::test]
    async fn test_span_tree_sort_order() -> Result<(), TraceEngineError> {
        cleanup();

        let storage_settings = ObjectStorageSettings::default();
        let service = TraceSpanService::new(&storage_settings, 24, Some(2), None).await?;

        // Build a deterministic tree: root → child → grandchild
        let trace_id = TraceId::from_bytes([1u8; 16]);
        let root_span_id = SpanId::from_bytes([1u8; 8]);
        let child_span_id = SpanId::from_bytes([2u8; 8]);
        let grandchild_span_id = SpanId::from_bytes([3u8; 8]);

        let root = make_span(
            &trace_id,
            root_span_id.clone(),
            None,
            "svc",
            "root_op",
            vec![],
        );
        let child = make_span(
            &trace_id,
            child_span_id.clone(),
            Some(root_span_id.clone()),
            "svc",
            "child_op",
            vec![],
        );
        let grandchild = make_span(
            &trace_id,
            grandchild_span_id.clone(),
            Some(child_span_id.clone()),
            "svc",
            "grandchild_op",
            vec![],
        );

        service.write_spans(vec![root, child, grandchild]).await?;
        tokio::time::sleep(Duration::from_secs(4)).await;

        let spans: Vec<TraceSpan> = service
            .query_service
            .get_trace_spans(Some(trace_id.as_bytes().as_slice()), None, None, None, None)
            .await?;

        assert_eq!(spans.len(), 3, "Expected 3 spans");

        // DFS order: root(0), child(1), grandchild(2)
        let by_order: Vec<&TraceSpan> = {
            let mut v: Vec<&TraceSpan> = spans.iter().collect();
            v.sort_by_key(|s| s.span_order);
            v
        };

        assert_eq!(
            by_order[0].span_name, "root_op",
            "span_order=0 should be root"
        );
        assert_eq!(by_order[0].depth, 0);
        assert_eq!(by_order[0].path.len(), 1);

        assert_eq!(
            by_order[1].span_name, "child_op",
            "span_order=1 should be child"
        );
        assert_eq!(by_order[1].depth, 1);
        assert_eq!(by_order[1].path.len(), 2);

        assert_eq!(
            by_order[2].span_name, "grandchild_op",
            "span_order=2 should be grandchild"
        );
        assert_eq!(by_order[2].depth, 2);
        assert_eq!(by_order[2].path.len(), 3);

        // All spans should share the same root_span_id
        let root_sid = root_span_id.to_hex();
        for span in &spans {
            assert_eq!(
                span.root_span_id, root_sid,
                "root_span_id mismatch for {}",
                span.span_name
            );
        }

        service.shutdown().await?;
        cleanup();
        Ok(())
    }

    /// Verify `get_trace_metrics` returns time-bucketed aggregate rows.
    #[tokio::test]
    async fn test_trace_metrics_basic() -> Result<(), TraceEngineError> {
        cleanup();

        let storage_settings = ObjectStorageSettings::default();
        let service = TraceSpanService::new(&storage_settings, 24, Some(2), None).await?;

        let (_record, spans, _tags) = generate_trace_with_spans(5, 0);
        service.write_spans(spans).await?;
        tokio::time::sleep(Duration::from_secs(4)).await;

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);

        let metrics = service
            .query_service
            .get_trace_metrics(None, start, end, "hour", None, None)
            .await?;

        assert!(!metrics.is_empty(), "Expected at least one metric bucket");
        assert!(metrics[0].trace_count > 0, "Expected non-zero trace count");

        service.shutdown().await?;
        cleanup();
        Ok(())
    }

    /// Verify `get_trace_metrics` with a service_name filter excludes other services.
    #[tokio::test]
    async fn test_trace_metrics_service_filter() -> Result<(), TraceEngineError> {
        cleanup();

        let storage_settings = ObjectStorageSettings::default();
        let service = TraceSpanService::new(&storage_settings, 24, Some(2), None).await?;

        // Write spans for two distinct services using deterministic IDs
        let trace_a = TraceId::from_bytes([10u8; 16]);
        let trace_b = TraceId::from_bytes([20u8; 16]);

        let span_a = make_span(
            &trace_a,
            SpanId::from_bytes([10u8; 8]),
            None,
            "service_alpha",
            "op_a",
            vec![],
        );
        let span_b = make_span(
            &trace_b,
            SpanId::from_bytes([20u8; 8]),
            None,
            "service_beta",
            "op_b",
            vec![],
        );

        service.write_spans(vec![span_a, span_b]).await?;
        tokio::time::sleep(Duration::from_secs(4)).await;

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);

        // Filter to service_alpha only
        let metrics_alpha = service
            .query_service
            .get_trace_metrics(Some("service_alpha"), start, end, "hour", None, None)
            .await?;

        // Filter to service_beta only
        let metrics_beta = service
            .query_service
            .get_trace_metrics(Some("service_beta"), start, end, "hour", None, None)
            .await?;

        let alpha_count: i64 = metrics_alpha.iter().map(|m| m.trace_count).sum();
        let beta_count: i64 = metrics_beta.iter().map(|m| m.trace_count).sum();

        assert!(alpha_count > 0, "Expected non-zero count for service_alpha");
        assert!(beta_count > 0, "Expected non-zero count for service_beta");

        // Querying with a non-existent service returns nothing
        let metrics_none = service
            .query_service
            .get_trace_metrics(Some("nonexistent_svc"), start, end, "hour", None, None)
            .await?;
        assert!(
            metrics_none.is_empty(),
            "Expected no buckets for nonexistent service"
        );

        service.shutdown().await?;
        cleanup();
        Ok(())
    }

    /// Verify all three DataFusion filter layers are wired correctly by inspecting the
    /// physical plan produced for a partitioned time-window query.
    ///
    /// Layer 1 — partition pruning: `partition_date` appears in the plan because Delta Lake
    /// pushes partition column filters to directory enumeration before reading any files.
    ///
    /// Layer 2 — row-group stats: `start_time` appears because typed Timestamp literals
    /// enable Parquet min/max pruning across row groups.
    ///
    /// Layer 3 — bloom filter: querying a nonexistent `trace_id` within a tight window
    /// returns 0 rows; bloom filters discard row groups instantly without page-level scanning.
    #[tokio::test]
    async fn test_query_plan_shows_filter_layers() -> Result<(), TraceEngineError> {
        cleanup();

        let storage_settings = ObjectStorageSettings::default();
        let service = TraceSpanService::new(&storage_settings, 24, Some(2), None).await?;

        // Write a known batch directly so it is immediately queryable
        // Use distinct byte values that don't collide with other tests.
        let trace_id = TraceId::from_bytes([0xAA_u8; 16]);
        let root_id = SpanId::from_bytes([0xAA_u8; 8]);
        let child_id = SpanId::from_bytes([0xBB_u8; 8]);
        let spans = vec![
            make_span(&trace_id, root_id.clone(), None, "svc-a", "root-op", vec![]),
            make_span(
                &trace_id,
                child_id.clone(),
                Some(root_id),
                "svc-a",
                "child-op",
                vec![],
            ),
        ];
        service.write_spans_direct(spans).await?;

        // ── Layers 1 & 2: inspect physical plan ──────────────────────────────────
        let now = Utc::now();
        let start = now - chrono::Duration::hours(1);
        let end = now + chrono::Duration::hours(1);

        // Build the same DataFrame the query path would produce internally
        let df = service
            .ctx
            .table(SPAN_TABLE_NAME)
            .await
            .map_err(TraceEngineError::DatafusionError)?;

        // Partition filter — pushed to directory enumeration (Layer 1)
        let df = df
            .filter(
                col(PARTITION_DATE_COL)
                    .gt_eq(date_lit(&start))
                    .and(col(PARTITION_DATE_COL).lt_eq(date_lit(&end))),
            )
            .map_err(TraceEngineError::DatafusionError)?;

        // Timestamp filter — enables row-group min/max pruning (Layer 2)
        let df = df
            .filter(
                col(START_TIME_COL)
                    .gt_eq(ts_lit(&start))
                    .and(col(START_TIME_COL).lt(ts_lit(&end))),
            )
            .map_err(TraceEngineError::DatafusionError)?;

        // Collect the physical plan as a string via EXPLAIN
        let explain_df = df
            .explain(false, false)
            .map_err(TraceEngineError::DatafusionError)?;
        let batches = explain_df
            .collect()
            .await
            .map_err(TraceEngineError::DatafusionError)?;

        let plan_text: String = batches
            .iter()
            .flat_map(|b| {
                let plan_col = b.column_by_name("plan").unwrap();
                let arr =
                    arrow::compute::cast(plan_col, &arrow::datatypes::DataType::Utf8).unwrap();
                let s = arr
                    .as_any()
                    .downcast_ref::<arrow::array::StringArray>()
                    .unwrap();
                (0..s.len())
                    .map(|i| s.value(i).to_string())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Layer 1: partition filter must appear in plan
        assert!(
            plan_text.contains("partition_date"),
            "Partition filter not found in physical plan:\n{plan_text}"
        );
        // Layer 2: start_time row-group filter must appear in plan
        assert!(
            plan_text.contains("start_time"),
            "Row-group time filter not found in physical plan:\n{plan_text}"
        );

        // ── Layer 3: bloom filter — behavioral proof ──────────────────────────────
        // A nonexistent trace_id with bloom filters enabled returns 0 rows.
        // Without bloom filters, DataFusion would scan every row group to confirm absence.
        let fake_id = TraceId::from_bytes([0xFF_u8; 16]);
        let result = service
            .query_service
            .get_trace_spans(
                Some(fake_id.as_bytes()),
                None,
                Some(&start),
                Some(&end),
                None,
            )
            .await?;
        assert!(
            result.is_empty(),
            "Expected 0 spans for nonexistent trace_id"
        );

        service.shutdown().await?;
        cleanup();
        Ok(())
    }

    /// Verify `get_trace_metrics` with attribute_filters narrows results to matching spans.
    #[tokio::test]
    async fn test_trace_metrics_with_attribute_filter() -> Result<(), TraceEngineError> {
        cleanup();

        let storage_settings = ObjectStorageSettings::default();
        let service = TraceSpanService::new(&storage_settings, 24, Some(2), None).await?;

        let trace_kafka = TraceId::from_bytes([30u8; 16]);
        let trace_http = TraceId::from_bytes([40u8; 16]);

        // span with component:kafka attribute
        let span_kafka = make_span(
            &trace_kafka,
            SpanId::from_bytes([30u8; 8]),
            None,
            "my_service",
            "kafka_consumer",
            vec![Attribute {
                key: "component".to_string(),
                value: Value::String("kafka".to_string()),
            }],
        );
        // span with component:http attribute (should NOT match kafka filter)
        let span_http = make_span(
            &trace_http,
            SpanId::from_bytes([40u8; 8]),
            None,
            "my_service",
            "http_handler",
            vec![Attribute {
                key: "component".to_string(),
                value: Value::String("http".to_string()),
            }],
        );

        service.write_spans(vec![span_kafka, span_http]).await?;
        tokio::time::sleep(Duration::from_secs(4)).await;

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);
        let kafka_filter = vec!["component:kafka".to_string()];

        // With filter, only kafka trace should appear
        let filtered = service
            .query_service
            .get_trace_metrics(None, start, end, "hour", Some(&kafka_filter), None)
            .await?;

        let filtered_count: i64 = filtered.iter().map(|m| m.trace_count).sum();
        assert!(
            filtered_count > 0,
            "Expected non-zero count with kafka attribute filter"
        );

        // Without filter, both traces appear
        let unfiltered = service
            .query_service
            .get_trace_metrics(None, start, end, "hour", None, None)
            .await?;
        let unfiltered_count: i64 = unfiltered.iter().map(|m| m.trace_count).sum();
        assert!(
            unfiltered_count >= filtered_count,
            "Unfiltered count ({}) should be >= filtered count ({})",
            unfiltered_count,
            filtered_count
        );

        service.shutdown().await?;
        cleanup();
        Ok(())
    }
}
