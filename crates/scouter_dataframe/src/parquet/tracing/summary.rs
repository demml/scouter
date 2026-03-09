use crate::error::TraceEngineError;
use crate::parquet::control::{get_pod_id, ControlTableEngine};
use crate::parquet::tracing::traits::arrow_schema_to_delta;
use crate::storage::ObjectStore;
use arrow::array::*;
use arrow::compute;
use arrow::datatypes::*;
use arrow_array::Array;
use arrow_array::RecordBatch;
use chrono::{DateTime, TimeZone, Utc};
use datafusion::logical_expr::{col, lit};
use datafusion::prelude::*;
use datafusion::scalar::ScalarValue;
use deltalake::operations::optimize::OptimizeType;
use deltalake::DeltaTable;
use scouter_settings::ObjectStorageSettings;
use scouter_types::sql::{TraceFilters, TraceListItem};
use scouter_types::{Attribute, TraceCursor, TraceId, TracePaginationResponse, TraceSummaryRecord};
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::sync::{mpsc, RwLock as AsyncRwLock};
use tokio::time::{interval, Duration};
use tracing::{error, info};
use url::Url;

const SUMMARY_TABLE_NAME: &str = "trace_summaries";

/// Control table task name for summary compaction coordination.
const TASK_SUMMARY_OPTIMIZE: &str = "summary_optimize";

// ── Column name constants ────────────────────────────────────────────────────
const BUCKET_TIME_COL: &str = "bucket_time";
const TRACE_ID_COL: &str = "trace_id";
const SERVICE_NAME_COL: &str = "service_name";
const SCOPE_NAME_COL: &str = "scope_name";
const SCOPE_VERSION_COL: &str = "scope_version";
const ROOT_OPERATION_COL: &str = "root_operation";
const START_TIME_COL: &str = "start_time";
const END_TIME_COL: &str = "end_time";
const DURATION_MS_COL: &str = "duration_ms";
const STATUS_CODE_COL: &str = "status_code";
const STATUS_MESSAGE_COL: &str = "status_message";
const SPAN_COUNT_COL: &str = "span_count";
const ERROR_COUNT_COL: &str = "error_count";
const SEARCH_BLOB_COL: &str = "search_blob";

const RESOURCE_ATTRIBUTES_COL: &str = "resource_attributes";
const ENTITY_ID_COL: &str = "entity_id";

const SUMMARY_COLUMNS: &[&str] = &[
    BUCKET_TIME_COL,
    TRACE_ID_COL,
    SERVICE_NAME_COL,
    SCOPE_NAME_COL,
    SCOPE_VERSION_COL,
    ROOT_OPERATION_COL,
    START_TIME_COL,
    END_TIME_COL,
    DURATION_MS_COL,
    STATUS_CODE_COL,
    STATUS_MESSAGE_COL,
    SPAN_COUNT_COL,
    ERROR_COUNT_COL,
    RESOURCE_ATTRIBUTES_COL,
    ENTITY_ID_COL,
];

// ── Schema ───────────────────────────────────────────────────────────────────

fn create_summary_schema() -> Schema {
    Schema::new(vec![
        Field::new(
            BUCKET_TIME_COL,
            DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
            false,
        ),
        Field::new(TRACE_ID_COL, DataType::FixedSizeBinary(16), false),
        Field::new(
            SERVICE_NAME_COL,
            DataType::Dictionary(Box::new(DataType::Int32), Box::new(DataType::Utf8)),
            false,
        ),
        Field::new(SCOPE_NAME_COL, DataType::Utf8, false),
        Field::new(SCOPE_VERSION_COL, DataType::Utf8, true),
        Field::new(ROOT_OPERATION_COL, DataType::Utf8, false),
        Field::new(
            START_TIME_COL,
            DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
            false,
        ),
        Field::new(
            END_TIME_COL,
            DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
            true,
        ),
        Field::new(DURATION_MS_COL, DataType::Int64, true),
        Field::new(STATUS_CODE_COL, DataType::Int32, false),
        Field::new(STATUS_MESSAGE_COL, DataType::Utf8, true),
        Field::new(SPAN_COUNT_COL, DataType::Int64, false),
        Field::new(ERROR_COUNT_COL, DataType::Int64, false),
        Field::new(RESOURCE_ATTRIBUTES_COL, DataType::Utf8, true),
        Field::new(ENTITY_ID_COL, DataType::Utf8, true),
    ])
}

/// Compute hour-truncated bucket_time from start_time (matches Postgres behavior).
fn bucket_time(start: DateTime<Utc>) -> DateTime<Utc> {
    use chrono::Timelike;
    start
        .with_minute(0)
        .unwrap()
        .with_second(0)
        .unwrap()
        .with_nanosecond(0)
        .unwrap()
}

// ── BatchBuilder ─────────────────────────────────────────────────────────────

struct TraceSummaryBatchBuilder {
    schema: Arc<Schema>,
    bucket_time: TimestampMicrosecondBuilder,
    trace_id: FixedSizeBinaryBuilder,
    service_name: StringDictionaryBuilder<Int32Type>,
    scope_name: StringBuilder,
    scope_version: StringBuilder,
    root_operation: StringBuilder,
    start_time: TimestampMicrosecondBuilder,
    end_time: TimestampMicrosecondBuilder,
    duration_ms: Int64Builder,
    status_code: Int32Builder,
    status_message: StringBuilder,
    span_count: Int64Builder,
    error_count: Int64Builder,
    resource_attributes: StringBuilder,
    entity_id: StringBuilder,
}

impl TraceSummaryBatchBuilder {
    fn new(schema: Arc<Schema>, capacity: usize) -> Self {
        Self {
            schema,
            bucket_time: TimestampMicrosecondBuilder::with_capacity(capacity).with_timezone("UTC"),
            trace_id: FixedSizeBinaryBuilder::with_capacity(capacity, 16),
            service_name: StringDictionaryBuilder::new(),
            scope_name: StringBuilder::with_capacity(capacity, capacity * 16),
            scope_version: StringBuilder::with_capacity(capacity, capacity * 8),
            root_operation: StringBuilder::with_capacity(capacity, capacity * 32),
            start_time: TimestampMicrosecondBuilder::with_capacity(capacity).with_timezone("UTC"),
            end_time: TimestampMicrosecondBuilder::with_capacity(capacity).with_timezone("UTC"),
            duration_ms: Int64Builder::with_capacity(capacity),
            status_code: Int32Builder::with_capacity(capacity),
            status_message: StringBuilder::with_capacity(capacity, capacity * 16),
            span_count: Int64Builder::with_capacity(capacity),
            error_count: Int64Builder::with_capacity(capacity),
            resource_attributes: StringBuilder::with_capacity(capacity, capacity * 64),
            entity_id: StringBuilder::with_capacity(capacity, capacity * 36),
        }
    }

    fn append(&mut self, rec: &TraceSummaryRecord) -> Result<(), TraceEngineError> {
        let btime = bucket_time(rec.start_time);
        self.bucket_time.append_value(btime.timestamp_micros());
        self.trace_id.append_value(rec.trace_id.as_bytes())?;
        self.service_name.append_value(&rec.service_name);
        self.scope_name.append_value(&rec.scope_name);
        if rec.scope_version.is_empty() {
            self.scope_version.append_null();
        } else {
            self.scope_version.append_value(&rec.scope_version);
        }
        self.root_operation.append_value(&rec.root_operation);
        self.start_time
            .append_value(rec.start_time.timestamp_micros());
        match rec.end_time {
            Some(end) => self.end_time.append_value(end.timestamp_micros()),
            None => self.end_time.append_null(),
        }
        let duration = rec
            .end_time
            .map(|end| (end - rec.start_time).num_milliseconds());
        match duration {
            Some(d) => self.duration_ms.append_value(d),
            None => self.duration_ms.append_null(),
        }
        self.status_code.append_value(rec.status_code);
        if rec.status_message.is_empty() {
            self.status_message.append_null();
        } else {
            self.status_message.append_value(&rec.status_message);
        }
        self.span_count.append_value(rec.span_count);
        self.error_count.append_value(rec.error_count);
        if rec.resource_attributes.is_empty() {
            self.resource_attributes.append_null();
        } else {
            let json = serde_json::to_string(&rec.resource_attributes)
                .unwrap_or_else(|_| "[]".to_string());
            self.resource_attributes.append_value(&json);
        }
        match rec.entity_id.as_deref() {
            Some(uid) => self.entity_id.append_value(uid),
            None => self.entity_id.append_null(),
        }
        Ok(())
    }

    fn finish(mut self) -> Result<RecordBatch, TraceEngineError> {
        let columns: Vec<Arc<dyn Array>> = vec![
            Arc::new(self.bucket_time.finish()),
            Arc::new(self.trace_id.finish()),
            Arc::new(self.service_name.finish()),
            Arc::new(self.scope_name.finish()),
            Arc::new(self.scope_version.finish()),
            Arc::new(self.root_operation.finish()),
            Arc::new(self.start_time.finish()),
            Arc::new(self.end_time.finish()),
            Arc::new(self.duration_ms.finish()),
            Arc::new(self.status_code.finish()),
            Arc::new(self.status_message.finish()),
            Arc::new(self.span_count.finish()),
            Arc::new(self.error_count.finish()),
            Arc::new(self.resource_attributes.finish()),
            Arc::new(self.entity_id.finish()),
        ];
        RecordBatch::try_new(self.schema, columns).map_err(Into::into)
    }
}

// ── TableCommand ─────────────────────────────────────────────────────────────

pub enum SummaryTableCommand {
    Write {
        records: Vec<TraceSummaryRecord>,
        respond_to: oneshot::Sender<Result<(), TraceEngineError>>,
    },
    Optimize {
        respond_to: oneshot::Sender<Result<(), TraceEngineError>>,
    },
    Vacuum {
        retention_hours: u64,
        respond_to: oneshot::Sender<Result<(), TraceEngineError>>,
    },
    Shutdown,
}

// ── Engine ───────────────────────────────────────────────────────────────────

async fn build_summary_url(object_store: &ObjectStore) -> Result<Url, TraceEngineError> {
    let mut base = object_store.get_base_url()?;
    let mut path = base.path().to_string();
    if !path.ends_with('/') {
        path.push('/');
    }
    path.push_str("summaries");
    base.set_path(&path);
    Ok(base)
}

async fn create_summary_table(
    table_url: Url,
    schema: SchemaRef,
) -> Result<DeltaTable, TraceEngineError> {
    info!("Creating new Delta summary table at URL: {}", table_url);
    let table = DeltaTable::try_from_url(table_url).await?;
    let delta_fields = arrow_schema_to_delta(&schema);
    table
        .create()
        .with_table_name(SUMMARY_TABLE_NAME)
        .with_columns(delta_fields)
        .await
        .map_err(Into::into)
}

async fn build_or_create_summary_table(
    object_store: &ObjectStore,
    schema: SchemaRef,
) -> Result<DeltaTable, TraceEngineError> {
    let table_url = build_summary_url(object_store).await?;
    info!("Loading summary table at URL: {}", table_url);

    if table_url.scheme() == "file" {
        if let Ok(path) = table_url.to_file_path() {
            if !path.exists() {
                std::fs::create_dir_all(&path)?;
            }
        }
    }

    match DeltaTable::try_from_url(table_url.clone()).await {
        Ok(table) => {
            info!("Loaded existing summary table");
            Ok(table)
        }
        Err(deltalake::DeltaTableError::NotATable(_)) => {
            create_summary_table(table_url, schema).await
        }
        Err(e) => Err(e.into()),
    }
}

pub struct TraceSummaryDBEngine {
    schema: Arc<Schema>,
    table: Arc<AsyncRwLock<DeltaTable>>,
    pub ctx: Arc<SessionContext>,
    control: ControlTableEngine,
}

impl TraceSummaryDBEngine {
    /// Create a new `TraceSummaryDBEngine` using the provided shared `SessionContext`.
    ///
    /// The caller is responsible for passing a `SessionContext` that already has the object-store
    /// backend configured (e.g. the one from `TraceSpanDBEngine`). This ensures both
    /// `trace_spans` and `trace_summaries` live in the same context and can participate in
    /// JOIN queries.
    pub async fn new(
        storage_settings: &ObjectStorageSettings,
        ctx: Arc<SessionContext>,
    ) -> Result<Self, TraceEngineError> {
        let object_store = ObjectStore::new(storage_settings)?;
        let schema = Arc::new(create_summary_schema());
        let delta_table = build_or_create_summary_table(&object_store, schema.clone()).await?;
        ctx.register_table(SUMMARY_TABLE_NAME, delta_table.table_provider().await?)?;

        let control = ControlTableEngine::new(storage_settings, get_pod_id()).await?;

        Ok(TraceSummaryDBEngine {
            schema,
            table: Arc::new(AsyncRwLock::new(delta_table)),
            ctx,
            control,
        })
    }

    fn build_batch(
        &self,
        records: Vec<TraceSummaryRecord>,
    ) -> Result<RecordBatch, TraceEngineError> {
        let mut builder = TraceSummaryBatchBuilder::new(self.schema.clone(), records.len());
        for rec in &records {
            builder.append(rec)?;
        }
        builder.finish()
    }

    async fn write_records(
        &self,
        records: Vec<TraceSummaryRecord>,
    ) -> Result<(), TraceEngineError> {
        let count = records.len();
        info!("Writing {} trace summaries", count);
        let batch = self.build_batch(records)?;

        let mut table_guard = self.table.write().await;

        if let Err(e) = table_guard.update_incremental(None).await {
            info!("Summary table update skipped (new table): {}", e);
        }

        let updated_table = table_guard
            .clone()
            .write(vec![batch])
            .with_save_mode(deltalake::protocol::SaveMode::Append)
            .await?;

        self.ctx.deregister_table(SUMMARY_TABLE_NAME)?;
        self.ctx
            .register_table(SUMMARY_TABLE_NAME, updated_table.table_provider().await?)?;

        *table_guard = updated_table;
        info!("Summary table updated with {} records", count);
        Ok(())
    }

    async fn optimize_table(&self) -> Result<(), TraceEngineError> {
        let mut table_guard = self.table.write().await;
        let (updated_table, _metrics) = table_guard
            .clone()
            .optimize()
            .with_target_size(128 * 1024 * 1024)
            .with_type(OptimizeType::ZOrder(vec![
                BUCKET_TIME_COL.to_string(),
                SERVICE_NAME_COL.to_string(),
            ]))
            .await?;

        self.ctx.deregister_table(SUMMARY_TABLE_NAME)?;
        self.ctx
            .register_table(SUMMARY_TABLE_NAME, updated_table.table_provider().await?)?;
        *table_guard = updated_table;
        Ok(())
    }

    async fn vacuum_table(&self, retention_hours: u64) -> Result<(), TraceEngineError> {
        let mut table_guard = self.table.write().await;
        let (updated_table, _metrics) = table_guard
            .clone()
            .vacuum()
            .with_retention_period(chrono::Duration::hours(retention_hours as i64))
            .with_enforce_retention_duration(false)
            .await?;

        self.ctx.deregister_table(SUMMARY_TABLE_NAME)?;
        self.ctx
            .register_table(SUMMARY_TABLE_NAME, updated_table.table_provider().await?)?;
        *table_guard = updated_table;
        Ok(())
    }

    /// Try to claim and run the summary optimize task via the control table.
    async fn try_run_optimize(&self, interval_hours: u64) {
        match self.control.try_claim_task(TASK_SUMMARY_OPTIMIZE).await {
            Ok(true) => match self.optimize_table().await {
                Ok(()) => {
                    let _ = self
                        .control
                        .release_task(
                            TASK_SUMMARY_OPTIMIZE,
                            chrono::Duration::hours(interval_hours as i64),
                        )
                        .await;
                }
                Err(e) => {
                    error!("Summary optimize failed: {}", e);
                    let _ = self
                        .control
                        .release_task_on_failure(TASK_SUMMARY_OPTIMIZE)
                        .await;
                }
            },
            Ok(false) => { /* not due or another pod owns it */ }
            Err(e) => error!("Summary optimize claim check failed: {}", e),
        }
    }

    pub fn start_actor(
        self,
        compaction_interval_hours: u64,
    ) -> (
        mpsc::Sender<SummaryTableCommand>,
        tokio::task::JoinHandle<()>,
    ) {
        let (tx, mut rx) = mpsc::channel::<SummaryTableCommand>(100);

        let handle = tokio::spawn(async move {
            // Poll every 5 minutes — the actual schedule is in the control table.
            let mut scheduler_ticker = interval(Duration::from_secs(5 * 60));
            scheduler_ticker.tick().await; // skip immediate tick

            loop {
                tokio::select! {
                    Some(cmd) = rx.recv() => {
                        match cmd {
                            SummaryTableCommand::Write { records, respond_to } => {
                                let result = self.write_records(records).await;
                                if let Err(ref e) = result {
                                    error!("Summary write failed: {}", e);
                                }
                                let _ = respond_to.send(result);
                            }
                            SummaryTableCommand::Optimize { respond_to } => {
                                // Direct admin request — bypass control table
                                let _ = respond_to.send(self.optimize_table().await);
                            }
                            SummaryTableCommand::Vacuum { retention_hours, respond_to } => {
                                let _ = respond_to.send(self.vacuum_table(retention_hours).await);
                            }
                            SummaryTableCommand::Shutdown => {
                                info!("TraceSummaryDBEngine actor shutting down");
                                break;
                            }
                        }
                    }
                    _ = scheduler_ticker.tick() => {
                        self.try_run_optimize(compaction_interval_hours).await;
                    }
                }
            }
        });

        (tx, handle)
    }
}

// ── Service ──────────────────────────────────────────────────────────────────

pub struct TraceSummaryService {
    engine_tx: mpsc::Sender<SummaryTableCommand>,
    engine_handle: tokio::task::JoinHandle<()>,
    pub query_service: TraceSummaryQueries,
}

impl TraceSummaryService {
    pub async fn new(
        storage_settings: &ObjectStorageSettings,
        compaction_interval_hours: u64,
        ctx: Arc<SessionContext>,
    ) -> Result<Self, TraceEngineError> {
        let engine = TraceSummaryDBEngine::new(storage_settings, ctx).await?;
        let engine_ctx = engine.ctx.clone();
        let (engine_tx, engine_handle) = engine.start_actor(compaction_interval_hours);

        Ok(TraceSummaryService {
            engine_tx,
            engine_handle,
            query_service: TraceSummaryQueries::new(engine_ctx),
        })
    }

    /// Write a batch of `TraceSummaryRecord`s to the Delta Lake summary table.
    pub async fn write_summaries(
        &self,
        records: Vec<TraceSummaryRecord>,
    ) -> Result<(), TraceEngineError> {
        let (tx, rx) = oneshot::channel();
        self.engine_tx
            .send(SummaryTableCommand::Write {
                records,
                respond_to: tx,
            })
            .await
            .map_err(|_| TraceEngineError::ChannelClosed)?;
        rx.await.map_err(|_| TraceEngineError::ChannelClosed)?
    }

    pub async fn optimize(&self) -> Result<(), TraceEngineError> {
        let (tx, rx) = oneshot::channel();
        self.engine_tx
            .send(SummaryTableCommand::Optimize { respond_to: tx })
            .await
            .map_err(|_| TraceEngineError::ChannelClosed)?;
        rx.await.map_err(|_| TraceEngineError::ChannelClosed)?
    }

    pub async fn vacuum(&self, retention_hours: u64) -> Result<(), TraceEngineError> {
        let (tx, rx) = oneshot::channel();
        self.engine_tx
            .send(SummaryTableCommand::Vacuum {
                retention_hours,
                respond_to: tx,
            })
            .await
            .map_err(|_| TraceEngineError::ChannelClosed)?;
        rx.await.map_err(|_| TraceEngineError::ChannelClosed)?
    }

    /// Signal shutdown without consuming `self` — safe to call from `Arc<TraceSummaryService>`.
    pub async fn signal_shutdown(&self) {
        info!("TraceSummaryService signaling shutdown");
        let _ = self.engine_tx.send(SummaryTableCommand::Shutdown).await;
    }

    pub async fn shutdown(self) -> Result<(), TraceEngineError> {
        info!("TraceSummaryService shutting down");
        self.engine_tx
            .send(SummaryTableCommand::Shutdown)
            .await
            .map_err(|_| TraceEngineError::ChannelClosed)?;
        if let Err(e) = self.engine_handle.await {
            error!("Summary engine handle error: {}", e);
        }
        info!("TraceSummaryService shutdown complete");
        Ok(())
    }
}

// ── Queries ──────────────────────────────────────────────────────────────────

pub struct TraceSummaryQueries {
    ctx: Arc<SessionContext>,
}

impl TraceSummaryQueries {
    pub fn new(ctx: Arc<SessionContext>) -> Self {
        Self { ctx }
    }

    /// Get paginated traces from the Delta Lake summary table.
    ///
    /// All filtering (time, service, errors, trace_ids, cursor) is pushed into DataFusion
    /// before collection. Attribute filters are resolved via a JOIN with the `trace_spans`
    /// table (requires shared `SessionContext` from `TraceSpanService`).
    /// Cursor pagination uses lexicographic `(start_time, trace_id)` comparison in DataFusion,
    /// and `LIMIT` is applied before `collect()` to bound data transfer.
    pub async fn get_paginated_traces(
        &self,
        filters: &TraceFilters,
    ) -> Result<TracePaginationResponse, TraceEngineError> {
        let limit = filters.limit.unwrap_or(25) as usize;
        let direction = filters.direction.as_deref().unwrap_or("next");

        let mut df = self.ctx.table(SUMMARY_TABLE_NAME).await?;
        df = df.select_columns(SUMMARY_COLUMNS)?;

        // ── Time filters FIRST for partition pruning ─────────────────────────
        if let Some(start) = filters.start_time {
            df = df.filter(
                col(START_TIME_COL).gt_eq(lit(ScalarValue::TimestampMicrosecond(
                    Some(start.timestamp_micros()),
                    Some("UTC".into()),
                ))),
            )?;
        }
        if let Some(end) = filters.end_time {
            df = df.filter(
                col(START_TIME_COL).lt(lit(ScalarValue::TimestampMicrosecond(
                    Some(end.timestamp_micros()),
                    Some("UTC".into()),
                ))),
            )?;
        }

        // ── Secondary filters ────────────────────────────────────────────────
        if let Some(ref svc) = filters.service_name {
            df = df.filter(col(SERVICE_NAME_COL).eq(lit(svc.as_str())))?;
        }
        match filters.has_errors {
            Some(true) => {
                df = df.filter(col(ERROR_COUNT_COL).gt(lit(0i64)))?;
            }
            Some(false) => {
                df = df.filter(col(ERROR_COUNT_COL).eq(lit(0i64)))?;
            }
            None => {}
        }
        if let Some(sc) = filters.status_code {
            df = df.filter(col(STATUS_CODE_COL).eq(lit(sc)))?;
        }

        // ── entity_uid direct column predicate ──────────────────────────────
        // Preferred over trace_ids IN-list: DataFusion uses per-file min/max for entity_id
        // to skip Parquet files — 90%+ file skipping after Z-ORDER compaction.
        if let Some(ref uid) = filters.entity_uid {
            df = df.filter(col(ENTITY_ID_COL).eq(lit(uid.as_str())))?;
        }

        // ── trace_ids IN filter ──────────────────────────────────────────────
        if let Some(ref ids) = filters.trace_ids {
            if !ids.is_empty() {
                let binary_ids: Vec<Expr> = ids
                    .iter()
                    .filter_map(|hex| TraceId::hex_to_bytes(hex).ok())
                    .map(|b| lit(ScalarValue::Binary(Some(b))))
                    .collect();
                if !binary_ids.is_empty() {
                    df = df.filter(col(TRACE_ID_COL).in_list(binary_ids, false))?;
                }
            }
        }

        // ── Cursor filter in DataFusion ──────────────────────────────────────
        // Equivalent to Postgres: `(start_time, trace_id) < (cursor_time, cursor_id)`
        // for "next" or `> (cursor_time, cursor_id)` for "previous".
        if let (Some(cursor_time), Some(ref cursor_id)) =
            (filters.cursor_start_time, &filters.cursor_trace_id)
        {
            if let Ok(cursor_bytes) = TraceId::hex_to_bytes(cursor_id) {
                let cursor_ts = lit(ScalarValue::TimestampMicrosecond(
                    Some(cursor_time.timestamp_micros()),
                    Some("UTC".into()),
                ));
                let cursor_tid = lit(ScalarValue::Binary(Some(cursor_bytes)));
                let cursor_expr = if direction == "previous" {
                    col(START_TIME_COL)
                        .gt(cursor_ts.clone())
                        .or(col(START_TIME_COL)
                            .eq(cursor_ts)
                            .and(col(TRACE_ID_COL).gt(cursor_tid)))
                } else {
                    col(START_TIME_COL)
                        .lt(cursor_ts.clone())
                        .or(col(START_TIME_COL)
                            .eq(cursor_ts)
                            .and(col(TRACE_ID_COL).lt(cursor_tid)))
                };
                df = df.filter(cursor_expr)?;
            }
        }

        // ── Attribute filters via span lookup → IN list ──────────────────────
        // Requires shared SessionContext (trace_spans must be registered in self.ctx).
        // We execute the span query eagerly to collect matching trace IDs, then filter
        // the summaries DataFrame with an IN-list predicate. This avoids a cross-table
        // JOIN that causes DataFusion to report ambiguous `trace_id` column references.
        if let Some(ref attr_filters) = filters.attribute_filters {
            if !attr_filters.is_empty() {
                let mut spans_df = self.ctx.table("trace_spans").await?.select_columns(&[
                    TRACE_ID_COL,
                    START_TIME_COL,
                    SEARCH_BLOB_COL,
                ])?;

                // Time predicates on spans for partition pruning
                if let Some(start) = filters.start_time {
                    spans_df = spans_df.filter(col(START_TIME_COL).gt_eq(lit(
                        ScalarValue::TimestampMicrosecond(
                            Some(start.timestamp_micros()),
                            Some("UTC".into()),
                        ),
                    )))?;
                }
                if let Some(end) = filters.end_time {
                    spans_df = spans_df.filter(col(START_TIME_COL).lt(lit(
                        ScalarValue::TimestampMicrosecond(
                            Some(end.timestamp_micros()),
                            Some("UTC".into()),
                        ),
                    )))?;
                }

                // OR-match each filter against search_blob.
                // normalize_attr_filter converts "key:value" → "%key=value%" so the LIKE
                // pattern matches the new pipe-bounded `|key=value|` blob format.
                let mut attr_expr: Option<Expr> = None;
                for f in attr_filters {
                    let pattern = crate::parquet::tracing::queries::normalize_attr_filter(f);
                    let cond = col(SEARCH_BLOB_COL).like(lit(pattern));
                    attr_expr = Some(match attr_expr {
                        None => cond,
                        Some(e) => e.or(cond),
                    });
                }
                if let Some(expr) = attr_expr {
                    spans_df = spans_df.filter(expr)?;
                }

                // Collect matching trace IDs eagerly, then apply as IN-list filter.
                // Use HashSet for O(1) dedup instead of O(n²) Vec::contains().
                let span_batches = spans_df.select_columns(&[TRACE_ID_COL])?.collect().await?;
                let mut seen_ids: std::collections::HashSet<Vec<u8>> =
                    std::collections::HashSet::new();
                let mut binary_ids: Vec<Expr> = Vec::new();
                for batch in &span_batches {
                    if let Some(col_arr) = batch
                        .column_by_name(TRACE_ID_COL)
                        .and_then(|c| c.as_any().downcast_ref::<BinaryArray>())
                    {
                        for i in 0..batch.num_rows() {
                            let id_bytes = col_arr.value(i).to_vec();
                            if seen_ids.insert(id_bytes.clone()) {
                                binary_ids.push(lit(ScalarValue::Binary(Some(id_bytes))));
                            }
                        }
                    }
                }

                if !binary_ids.is_empty() {
                    df = df.filter(col(TRACE_ID_COL).in_list(binary_ids, false))?;
                } else {
                    // No matching spans → return empty result
                    df = df.filter(lit(false))?;
                }
            }
        }

        // ── Sort: DESC for "next", ASC for "previous" ────────────────────────
        // "previous" direction fetches the oldest limit+1 items newer than the cursor,
        // which matches the original Rust post-reversal behavior.
        df = if direction == "previous" {
            df.sort(vec![
                col(START_TIME_COL).sort(true, true),
                col(TRACE_ID_COL).sort(true, true),
            ])?
        } else {
            df.sort(vec![
                col(START_TIME_COL).sort(false, false),
                col(TRACE_ID_COL).sort(false, false),
            ])?
        };

        // ── LIMIT pushed into DataFusion (fetch limit+1 to detect next page) ─
        df = df.limit(0, Some(limit + 1))?;

        let batches = df.collect().await?;
        let mut items = batches_to_trace_list_items(batches)?;

        let has_next = items.len() > limit;
        let has_previous = filters.cursor_start_time.is_some();

        if has_next {
            items.truncate(limit);
        }

        let next_cursor = if has_next {
            items.last().map(|item| TraceCursor {
                start_time: item.start_time,
                trace_id: item.trace_id.clone(),
            })
        } else {
            None
        };

        let previous_cursor = if has_previous {
            items.first().map(|item| TraceCursor {
                start_time: item.start_time,
                trace_id: item.trace_id.clone(),
            })
        } else {
            None
        };

        Ok(TracePaginationResponse {
            items,
            has_next,
            next_cursor,
            has_previous,
            previous_cursor,
        })
    }
}

// ── Arrow → TraceListItem conversion ─────────────────────────────────────────

fn batches_to_trace_list_items(
    batches: Vec<RecordBatch>,
) -> Result<Vec<TraceListItem>, TraceEngineError> {
    let mut items = Vec::new();

    for batch in &batches {
        // Delta Lake stores FixedSizeBinary(16) as Binary; DataFusion returns BinaryArray on read.
        let trace_ids = batch
            .column_by_name(TRACE_ID_COL)
            .and_then(|c| c.as_any().downcast_ref::<BinaryArray>())
            .ok_or_else(|| {
                TraceEngineError::UnsupportedOperation("missing trace_id column".into())
            })?;

        // Dictionary-encoded service_name — cast to Utf8 for simple access
        let service_name_col = batch.column_by_name(SERVICE_NAME_COL).ok_or_else(|| {
            TraceEngineError::UnsupportedOperation("missing service_name column".into())
        })?;
        let service_name_utf8 = compute::cast(service_name_col, &DataType::Utf8)?;
        let service_names = service_name_utf8
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or_else(|| {
                TraceEngineError::UnsupportedOperation(
                    "service_name cast to StringArray failed".into(),
                )
            })?;

        let scope_names = batch
            .column_by_name(SCOPE_NAME_COL)
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or_else(|| {
                TraceEngineError::UnsupportedOperation("missing scope_name column".into())
            })?;

        let scope_versions = batch
            .column_by_name(SCOPE_VERSION_COL)
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or_else(|| {
                TraceEngineError::UnsupportedOperation("missing scope_version column".into())
            })?;

        let root_operations = batch
            .column_by_name(ROOT_OPERATION_COL)
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or_else(|| {
                TraceEngineError::UnsupportedOperation("missing root_operation column".into())
            })?;

        let start_times = batch
            .column_by_name(START_TIME_COL)
            .and_then(|c| c.as_any().downcast_ref::<TimestampMicrosecondArray>())
            .ok_or_else(|| {
                TraceEngineError::UnsupportedOperation("missing start_time column".into())
            })?;

        let end_times = batch
            .column_by_name(END_TIME_COL)
            .and_then(|c| c.as_any().downcast_ref::<TimestampMicrosecondArray>())
            .ok_or_else(|| {
                TraceEngineError::UnsupportedOperation("missing end_time column".into())
            })?;

        let durations = batch
            .column_by_name(DURATION_MS_COL)
            .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
            .ok_or_else(|| {
                TraceEngineError::UnsupportedOperation("missing duration_ms column".into())
            })?;

        let status_codes = batch
            .column_by_name(STATUS_CODE_COL)
            .and_then(|c| c.as_any().downcast_ref::<Int32Array>())
            .ok_or_else(|| {
                TraceEngineError::UnsupportedOperation("missing status_code column".into())
            })?;

        let status_messages = batch
            .column_by_name(STATUS_MESSAGE_COL)
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or_else(|| {
                TraceEngineError::UnsupportedOperation("missing status_message column".into())
            })?;

        let span_counts = batch
            .column_by_name(SPAN_COUNT_COL)
            .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
            .ok_or_else(|| {
                TraceEngineError::UnsupportedOperation("missing span_count column".into())
            })?;

        let error_counts = batch
            .column_by_name(ERROR_COUNT_COL)
            .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
            .ok_or_else(|| {
                TraceEngineError::UnsupportedOperation("missing error_count column".into())
            })?;

        let resource_attrs_col = batch
            .column_by_name(RESOURCE_ATTRIBUTES_COL)
            .and_then(|c| c.as_any().downcast_ref::<StringArray>())
            .ok_or_else(|| {
                TraceEngineError::UnsupportedOperation("missing resource_attributes column".into())
            })?;

        for i in 0..batch.num_rows() {
            let trace_id_hex = hex::encode(trace_ids.value(i));

            let start_time = micros_to_datetime(start_times.value(i));
            let end_time = if end_times.is_null(i) {
                None
            } else {
                Some(micros_to_datetime(end_times.value(i)))
            };
            let duration_ms = if durations.is_null(i) {
                None
            } else {
                Some(durations.value(i))
            };
            let error_count = error_counts.value(i);

            let resource_attributes: Vec<Attribute> = if resource_attrs_col.is_null(i) {
                Vec::new()
            } else {
                serde_json::from_str(resource_attrs_col.value(i)).unwrap_or_default()
            };

            items.push(TraceListItem {
                trace_id: trace_id_hex,
                service_name: service_names.value(i).to_string(),
                scope_name: scope_names.value(i).to_string(),
                scope_version: scope_versions.value(i).to_string(),
                root_operation: root_operations.value(i).to_string(),
                start_time,
                end_time,
                duration_ms,
                status_code: status_codes.value(i),
                status_message: if status_messages.is_null(i) {
                    None
                } else {
                    Some(status_messages.value(i).to_string())
                },
                span_count: span_counts.value(i),
                has_errors: error_count > 0,
                error_count,
                resource_attributes,
            });
        }
    }

    Ok(items)
}

fn micros_to_datetime(micros: i64) -> DateTime<Utc> {
    let secs = micros / 1_000_000;
    let nanos = ((micros % 1_000_000) * 1_000) as u32;
    Utc.timestamp_opt(secs, nanos).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::ObjectStore;
    use scouter_settings::ObjectStorageSettings;
    use scouter_types::sql::TraceFilters;
    use scouter_types::{Attribute, SpanId, TraceId, TraceSpanRecord};
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

    /// Build a standalone SessionContext for test use (no trace_spans registered).
    /// Attribute-filter paths that need trace_spans are not exercised in these tests.
    fn make_test_ctx(storage_settings: &ObjectStorageSettings) -> Arc<SessionContext> {
        Arc::new(
            ObjectStore::new(storage_settings)
                .unwrap()
                .get_session()
                .unwrap(),
        )
    }

    fn make_summary(
        trace_id_bytes: [u8; 16],
        service_name: &str,
        error_count: i64,
        resource_attributes: Vec<Attribute>,
    ) -> TraceSummaryRecord {
        let now = Utc::now();
        TraceSummaryRecord {
            trace_id: TraceId::from_bytes(trace_id_bytes),
            service_name: service_name.to_string(),
            scope_name: "test.scope".to_string(),
            scope_version: String::new(),
            root_operation: "root_op".to_string(),
            start_time: now,
            end_time: Some(now + chrono::Duration::milliseconds(200)),
            status_code: if error_count > 0 { 2 } else { 0 },
            status_message: if error_count > 0 {
                "Internal Server Error".to_string()
            } else {
                "OK".to_string()
            },
            span_count: 3,
            error_count,
            resource_attributes,
            entity_id: None,
        }
    }

    /// Basic write + paginate round-trip: writes two summaries and verifies both appear.
    #[tokio::test]
    async fn test_summary_write_and_paginate_basic() -> Result<(), TraceEngineError> {
        cleanup();

        let storage_settings = ObjectStorageSettings::default();
        let ctx = make_test_ctx(&storage_settings);
        let service = TraceSummaryService::new(&storage_settings, 24, ctx).await?;

        let s1 = make_summary([1u8; 16], "svc_a", 0, vec![]);
        let s2 = make_summary([2u8; 16], "svc_b", 0, vec![]);
        service.write_summaries(vec![s1, s2]).await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);
        let filters = TraceFilters {
            service_name: None,
            has_errors: None,
            status_code: None,
            start_time: Some(start),
            end_time: Some(end),
            limit: Some(25),
            cursor_start_time: None,
            cursor_trace_id: None,
            direction: None,
            attribute_filters: None,
            trace_ids: None,
            entity_uid: None,
        };

        let response = service.query_service.get_paginated_traces(&filters).await?;
        assert!(
            response.items.len() >= 2,
            "Expected at least 2 items, got {}",
            response.items.len()
        );

        service.shutdown().await?;
        cleanup();
        Ok(())
    }

    /// `has_errors = Some(true)` returns only error traces; `Some(false)` returns only non-errors.
    #[tokio::test]
    async fn test_summary_has_errors_filter() -> Result<(), TraceEngineError> {
        cleanup();

        let storage_settings = ObjectStorageSettings::default();
        let ctx = make_test_ctx(&storage_settings);
        let service = TraceSummaryService::new(&storage_settings, 24, ctx).await?;

        let ok_summary = make_summary([3u8; 16], "svc", 0, vec![]);
        let err_summary = make_summary([4u8; 16], "svc", 2, vec![]);
        service
            .write_summaries(vec![ok_summary, err_summary])
            .await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);

        let base_filters = TraceFilters {
            service_name: None,
            has_errors: None,
            status_code: None,
            start_time: Some(start),
            end_time: Some(end),
            limit: Some(25),
            cursor_start_time: None,
            cursor_trace_id: None,
            direction: None,
            attribute_filters: None,
            trace_ids: None,
            entity_uid: None,
        };

        // has_errors = true → only error trace
        let mut filters_err = base_filters.clone();
        filters_err.has_errors = Some(true);
        let errors_only = service
            .query_service
            .get_paginated_traces(&filters_err)
            .await?;
        for item in &errors_only.items {
            assert!(
                item.error_count > 0,
                "Expected error trace, got: {:?}",
                item
            );
        }
        assert!(
            !errors_only.items.is_empty(),
            "Expected at least one error trace"
        );

        // has_errors = false → only non-error traces
        let mut filters_ok = base_filters.clone();
        filters_ok.has_errors = Some(false);
        let no_errors = service
            .query_service
            .get_paginated_traces(&filters_ok)
            .await?;
        for item in &no_errors.items {
            assert_eq!(
                item.error_count, 0,
                "Expected non-error trace, got error_count={}",
                item.error_count
            );
        }
        assert!(
            !no_errors.items.is_empty(),
            "Expected at least one non-error trace"
        );

        service.shutdown().await?;
        cleanup();
        Ok(())
    }

    /// service_name filter returns only matching service traces.
    #[tokio::test]
    async fn test_summary_service_name_filter() -> Result<(), TraceEngineError> {
        cleanup();

        let storage_settings = ObjectStorageSettings::default();
        let ctx = make_test_ctx(&storage_settings);
        let service = TraceSummaryService::new(&storage_settings, 24, ctx).await?;

        let s_alpha = make_summary([5u8; 16], "alpha_service", 0, vec![]);
        let s_beta = make_summary([6u8; 16], "beta_service", 0, vec![]);
        service.write_summaries(vec![s_alpha, s_beta]).await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);
        let filters = TraceFilters {
            service_name: Some("alpha_service".to_string()),
            has_errors: None,
            status_code: None,
            start_time: Some(start),
            end_time: Some(end),
            limit: Some(25),
            cursor_start_time: None,
            cursor_trace_id: None,
            direction: None,
            attribute_filters: None,
            trace_ids: None,
            entity_uid: None,
        };

        let response = service.query_service.get_paginated_traces(&filters).await?;
        assert!(
            !response.items.is_empty(),
            "Expected results for alpha_service"
        );
        for item in &response.items {
            assert_eq!(
                item.service_name, "alpha_service",
                "Expected only alpha_service items, got: {}",
                item.service_name
            );
        }

        service.shutdown().await?;
        cleanup();
        Ok(())
    }

    /// trace_ids IN filter returns only the specified traces.
    #[tokio::test]
    async fn test_summary_trace_ids_filter() -> Result<(), TraceEngineError> {
        cleanup();

        let storage_settings = ObjectStorageSettings::default();
        let ctx = make_test_ctx(&storage_settings);
        let service = TraceSummaryService::new(&storage_settings, 24, ctx).await?;

        let wanted_id = TraceId::from_bytes([7u8; 16]);
        let unwanted_id = TraceId::from_bytes([8u8; 16]);

        let s1 = make_summary([7u8; 16], "svc", 0, vec![]);
        let s2 = make_summary([8u8; 16], "svc", 0, vec![]);
        service.write_summaries(vec![s1, s2]).await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);
        let filters = TraceFilters {
            service_name: None,
            has_errors: None,
            status_code: None,
            start_time: Some(start),
            end_time: Some(end),
            limit: Some(25),
            cursor_start_time: None,
            cursor_trace_id: None,
            direction: None,
            attribute_filters: None,
            trace_ids: Some(vec![wanted_id.to_hex()]),
            entity_uid: None,
        };

        let response = service.query_service.get_paginated_traces(&filters).await?;
        assert_eq!(
            response.items.len(),
            1,
            "Expected exactly 1 item from trace_ids filter"
        );
        assert_eq!(
            response.items[0].trace_id,
            wanted_id.to_hex(),
            "Returned wrong trace_id"
        );
        assert_ne!(
            response.items[0].trace_id,
            unwanted_id.to_hex(),
            "Should not have returned unwanted trace_id"
        );

        service.shutdown().await?;
        cleanup();
        Ok(())
    }

    /// Cursor pagination: first page → next → previous all return correct item counts.
    #[tokio::test]
    async fn test_summary_cursor_pagination() -> Result<(), TraceEngineError> {
        cleanup();
        let storage_settings = ObjectStorageSettings::default();
        let ctx = make_test_ctx(&storage_settings);
        let service = TraceSummaryService::new(&storage_settings, 24, ctx).await?;

        let now = Utc::now();
        let summaries: Vec<TraceSummaryRecord> = (0u8..100)
            .map(|i| {
                let mut s = make_summary([i; 16], "svc", 0, vec![]);
                s.start_time = now - chrono::Duration::minutes(i as i64);
                s
            })
            .collect();
        service.write_summaries(summaries).await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        let mut filters = TraceFilters {
            start_time: Some(now - chrono::Duration::hours(2)),
            end_time: Some(now + chrono::Duration::hours(1)),
            limit: Some(50),
            ..Default::default()
        };

        // First page
        let first = service.query_service.get_paginated_traces(&filters).await?;
        assert_eq!(first.items.len(), 50, "first page: 50 items");
        assert!(
            first.next_cursor.is_some(),
            "first page: should have next_cursor"
        );

        // Next page
        let next_cur = first.next_cursor.clone().unwrap();
        filters.cursor_start_time = Some(next_cur.start_time);
        filters.cursor_trace_id = Some(next_cur.trace_id.clone());
        filters.direction = Some("next".to_string());
        let second = service.query_service.get_paginated_traces(&filters).await?;
        assert_eq!(second.items.len(), 50, "second page: 50 items");
        assert!(
            second.items[0].start_time <= next_cur.start_time,
            "second page first item must be <= cursor"
        );
        assert!(second.previous_cursor.is_some());

        // Previous page
        let prev_cur = second.previous_cursor.unwrap();
        filters.cursor_start_time = Some(prev_cur.start_time);
        filters.cursor_trace_id = Some(prev_cur.trace_id.clone());
        filters.direction = Some("previous".to_string());
        let prev = service.query_service.get_paginated_traces(&filters).await?;
        assert_eq!(prev.items.len(), 50, "previous page: 50 items");

        service.shutdown().await?;
        cleanup();
        Ok(())
    }

    /// Attribute-filter JOIN path: only traces with matching span attributes are returned.
    #[tokio::test]
    async fn test_summary_attribute_filter_via_join() -> Result<(), TraceEngineError> {
        use crate::parquet::tracing::service::TraceSpanService;

        cleanup();
        let storage_settings = ObjectStorageSettings::default();

        // TraceSpanService owns the SessionContext (trace_spans registered in it)
        let span_service = TraceSpanService::new(&storage_settings, 24, Some(2), None).await?;
        let shared_ctx = span_service.ctx.clone();

        // TraceSummaryService shares the same ctx — JOIN to trace_spans will work
        let summary_service = TraceSummaryService::new(&storage_settings, 24, shared_ctx).await?;

        let now = Utc::now();
        let kafka_trace = TraceId::from_bytes([70u8; 16]);
        let plain_trace = TraceId::from_bytes([80u8; 16]);

        let kafka_span = make_span_record(
            &kafka_trace,
            SpanId::from_bytes([70u8; 8]),
            "svc",
            vec![Attribute {
                key: "component".to_string(),
                value: serde_json::Value::String("kafka".to_string()),
            }],
        );
        let plain_span =
            make_span_record(&plain_trace, SpanId::from_bytes([80u8; 8]), "svc", vec![]);
        span_service
            .write_spans(vec![kafka_span, plain_span])
            .await?;

        let mut kafka_summary = make_summary([70u8; 16], "svc", 0, vec![]);
        kafka_summary.start_time = now;
        let mut plain_summary = make_summary([80u8; 16], "svc", 0, vec![]);
        plain_summary.start_time = now;
        summary_service
            .write_summaries(vec![kafka_summary, plain_summary])
            .await?;

        tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;

        let filters = TraceFilters {
            start_time: Some(now - chrono::Duration::hours(1)),
            end_time: Some(now + chrono::Duration::hours(1)),
            attribute_filters: Some(vec!["component:kafka".to_string()]),
            limit: Some(25),
            ..Default::default()
        };

        let response = summary_service
            .query_service
            .get_paginated_traces(&filters)
            .await?;

        assert!(
            !response.items.is_empty(),
            "attribute filter must return results"
        );
        assert!(
            response
                .items
                .iter()
                .all(|i| i.trace_id == kafka_trace.to_hex()),
            "only kafka trace should appear; got {:?}",
            response
                .items
                .iter()
                .map(|i| &i.trace_id)
                .collect::<Vec<_>>()
        );

        span_service.shutdown().await?;
        summary_service.shutdown().await?;
        cleanup();
        Ok(())
    }

    /// Build a deterministic `TraceSpanRecord` for use in summary tests.
    fn make_span_record(
        trace_id: &TraceId,
        span_id: SpanId,
        service_name: &str,
        attributes: Vec<Attribute>,
    ) -> TraceSpanRecord {
        let now = Utc::now();
        TraceSpanRecord {
            created_at: now,
            trace_id: trace_id.clone(),
            span_id,
            parent_span_id: None,
            flags: 1,
            trace_state: String::new(),
            scope_name: "test.scope".to_string(),
            scope_version: None,
            span_name: "op".to_string(),
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
            input: serde_json::Value::Null,
            output: serde_json::Value::Null,
            service_name: service_name.to_string(),
            resource_attributes: vec![],
        }
    }

    /// `resource_attributes` survive a write → read round-trip.
    #[tokio::test]
    async fn test_summary_resource_attributes_roundtrip() -> Result<(), TraceEngineError> {
        cleanup();

        let storage_settings = ObjectStorageSettings::default();
        let ctx = make_test_ctx(&storage_settings);
        let service = TraceSummaryService::new(&storage_settings, 24, ctx).await?;

        let attrs = vec![Attribute {
            key: "cloud.region".to_string(),
            value: serde_json::Value::String("us-east-1".to_string()),
        }];
        let summary = make_summary([9u8; 16], "svc", 0, attrs.clone());
        service.write_summaries(vec![summary]).await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        let start = Utc::now() - chrono::Duration::hours(1);
        let end = Utc::now() + chrono::Duration::hours(1);
        let filters = TraceFilters {
            service_name: None,
            has_errors: None,
            status_code: None,
            start_time: Some(start),
            end_time: Some(end),
            limit: Some(25),
            cursor_start_time: None,
            cursor_trace_id: None,
            direction: None,
            attribute_filters: None,
            trace_ids: Some(vec![TraceId::from_bytes([9u8; 16]).to_hex()]),
            entity_uid: None,
        };

        let response = service.query_service.get_paginated_traces(&filters).await?;
        assert_eq!(response.items.len(), 1, "Expected exactly 1 item");
        assert_eq!(
            response.items[0].resource_attributes.len(),
            1,
            "Expected 1 resource attribute"
        );
        assert_eq!(response.items[0].resource_attributes[0].key, "cloud.region");

        service.shutdown().await?;
        cleanup();
        Ok(())
    }
}
