use crate::error::TraceEngineError;
use crate::parquet::control::{get_pod_id, ControlTableEngine};
use crate::parquet::tracing::traits::arrow_schema_to_delta;
use crate::parquet::tracing::traits::attribute_field;
use crate::parquet::tracing::traits::TraceSchemaExt;
use crate::storage::ObjectStore;
use arrow::array::*;
use arrow::datatypes::*;
use arrow_array::RecordBatch;
use chrono::{Datelike, Utc};
use datafusion::prelude::SessionContext;
use deltalake::datafusion::parquet::basic::{Compression, Encoding, ZstdLevel};
use deltalake::datafusion::parquet::file::properties::{EnabledStatistics, WriterProperties};
use deltalake::datafusion::parquet::schema::types::ColumnPath;
use deltalake::logstore::{
    default_logstore, logstore_factories, LogStore, LogStoreFactory, ObjectStoreRef, StorageConfig,
};
use deltalake::operations::optimize::OptimizeType;
use deltalake::{DeltaResult, DeltaTable, DeltaTableBuilder, TableProperty};
use scouter_settings::ObjectStorageSettings;
use scouter_types::SpanId;
use scouter_types::TraceId;
use scouter_types::TraceSpanRecord;
use scouter_types::{Attribute, SpanEvent, SpanLink, SCOUTER_ENTITY};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::sync::{mpsc, RwLock as AsyncRwLock};
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, instrument};
use url::Url;

const TRACE_SPAN_TABLE_NAME: &str = "trace_spans";

/// Control table task names for distributed coordination.
const TASK_OPTIMIZE: &str = "trace_optimize";
const TASK_RETENTION: &str = "trace_retention";

/// Days from year-0001 to Unix epoch (1970-01-01), used to convert chrono → Arrow Date32.
/// Equivalent to `NaiveDate::from_ymd_opt(1970, 1, 1).unwrap().num_days_from_ce()`.
const UNIX_EPOCH_DAYS: i32 = 719_163;

pub enum TableCommand {
    Write {
        spans: Vec<TraceSpanRecord>,
        respond_to: oneshot::Sender<Result<(), TraceEngineError>>,
    },
    Optimize {
        respond_to: oneshot::Sender<Result<(), TraceEngineError>>,
    },
    Vacuum {
        retention_hours: u64,
        respond_to: oneshot::Sender<Result<(), TraceEngineError>>,
    },
    Expire {
        cutoff_date: chrono::NaiveDate,
        respond_to: oneshot::Sender<Result<(), TraceEngineError>>,
    },
    Shutdown,
}

struct PassthroughLogStoreFactory;

impl LogStoreFactory for PassthroughLogStoreFactory {
    fn with_options(
        &self,
        prefixed_store: ObjectStoreRef,
        root_store: ObjectStoreRef,
        location: &Url,
        options: &StorageConfig,
    ) -> DeltaResult<Arc<dyn LogStore>> {
        // For az:// URLs, object_store's ObjectStoreScheme::parse uses strip_bucket()
        // which assumes az://account/container/blob-path format. Scouter uses
        // az://container/blob-path (container in host, subpath in URL path).
        // strip_bucket() finds no second path segment → returns "" → delta-rs
        // applies no PrefixStore for Azure. Manually apply the correct prefix here.
        //
        // For gs://, s3://, s3a://, abfs://, abfss:// — delta-rs correctly derives
        // the subpath prefix from url.path() and applies PrefixStore via decorate_prefix.
        // Do not re-wrap those: use the already-prefixed `prefixed_store` as-is.
        let store = if location.scheme() == "az" {
            let subpath = location.path().trim_start_matches('/');
            if subpath.is_empty() {
                prefixed_store
            } else {
                let prefix = object_store::path::Path::from(subpath);
                Arc::new(object_store::prefix::PrefixStore::new(
                    root_store.clone(),
                    prefix,
                )) as ObjectStoreRef
            }
        } else {
            prefixed_store
        };
        Ok(default_logstore(store, root_store, location, options))
    }
}

fn register_cloud_logstore_factories() {
    let factories = logstore_factories();
    let factory = Arc::new(PassthroughLogStoreFactory) as Arc<dyn LogStoreFactory>;
    for scheme in ["gs", "s3", "s3a", "az", "abfs", "abfss"] {
        let key = Url::parse(&format!("{}://", scheme)).expect("scheme is a valid URL prefix");
        if !factories.contains_key(&key) {
            factories.insert(key, factory.clone());
        }
    }
}

async fn build_url(object_store: &ObjectStore) -> Result<Url, TraceEngineError> {
    let base_url = object_store.get_base_url()?;
    Ok(base_url)
}

#[instrument(skip_all)]
async fn create_table(
    object_store: &ObjectStore,
    table_url: Url,
    schema: SchemaRef,
) -> Result<DeltaTable, TraceEngineError> {
    info!("Creating new Delta table at URL: {}", table_url);

    let store = object_store.as_dyn_object_store();
    let table = DeltaTableBuilder::from_url(table_url.clone())?
        .with_storage_backend(store, table_url)
        .build()?;

    let delta_fields = arrow_schema_to_delta(&schema);

    table
        .create()
        .with_table_name(TRACE_SPAN_TABLE_NAME)
        .with_columns(delta_fields)
        .with_partition_columns(vec!["partition_date".to_string()])
        .with_configuration_property(TableProperty::CheckpointInterval, Some("5"))
        // Only collect min/max statistics for columns that benefit from data skipping.
        .with_configuration_property(
            TableProperty::DataSkippingStatsColumns,
            Some(
                "start_time,end_time,service_name,duration_ms,status_code,entity_id,partition_date",
            ),
        )
        .await
        .map_err(Into::into)
}

#[instrument(skip_all)]
async fn build_or_create_table(
    object_store: &ObjectStore,
    schema: SchemaRef,
) -> Result<DeltaTable, TraceEngineError> {
    register_cloud_logstore_factories();
    let table_url = build_url(object_store).await?;
    info!("Attempting to load table at URL: {}", table_url);

    // For all store types we check for an existing Delta table by attempting a load.
    // Local tables can be checked cheaply via the filesystem; remote tables require
    // an actual load attempt against the object store.
    let is_delta_table = if table_url.scheme() == "file" {
        if let Ok(path) = table_url.to_file_path() {
            if !path.exists() {
                info!("Creating directory for local table: {:?}", path);
                std::fs::create_dir_all(&path)?;
            }
            path.join("_delta_log").exists()
        } else {
            false
        }
    } else {
        let store = object_store.as_dyn_object_store();
        match DeltaTableBuilder::from_url(table_url.clone()) {
            Ok(builder) => builder
                .with_storage_backend(store, table_url.clone())
                .load()
                .await
                .is_ok(),
            Err(_) => false,
        }
    };

    if is_delta_table {
        info!("Loading existing Delta table");
        let store = object_store.as_dyn_object_store();
        let table = DeltaTableBuilder::from_url(table_url.clone())?
            .with_storage_backend(store, table_url)
            .load()
            .await?;
        Ok(table)
    } else {
        info!("Table does not exist, creating new table");
        create_table(object_store, table_url, schema).await
    }
}

/// Core trace span engine for high-throughput observability workloads.
///
/// Hierarchy fields (depth, span_order, path, root_span_id) are NOT stored — they are
/// computed at query time via Rust DFS traversal. This matches how Jaeger/Zipkin operate and
/// avoids ordering dependencies during ingest (spans may arrive out-of-order within a batch).
pub struct TraceSpanDBEngine {
    schema: Arc<Schema>,
    pub object_store: ObjectStore,
    table: Arc<AsyncRwLock<DeltaTable>>,
    pub ctx: Arc<SessionContext>,
    control: ControlTableEngine,
}

impl TraceSchemaExt for TraceSpanDBEngine {}

impl TraceSpanDBEngine {
    pub async fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, TraceEngineError> {
        let object_store = ObjectStore::new(storage_settings)?;
        let schema = Arc::new(Self::create_schema());
        let delta_table = build_or_create_table(&object_store, schema.clone()).await?;
        let ctx = object_store.get_session()?;

        // A freshly-created table has no committed Parquet files yet — table_provider()
        // Defer registration until the first write populates the log.
        if let Ok(provider) = delta_table.table_provider().await {
            ctx.register_table(TRACE_SPAN_TABLE_NAME, provider)?;
        } else {
            info!("Empty table at init — deferring SessionContext registration until first write");
        }
        let control = ControlTableEngine::new(storage_settings, get_pod_id()).await?;

        Ok(TraceSpanDBEngine {
            schema,
            object_store,
            table: Arc::new(AsyncRwLock::new(delta_table)),
            ctx: Arc::new(ctx),
            control,
        })
    }

    /// Build a RecordBatch from a vector of TraceSpanRecord (raw ingest type, no hierarchy).
    pub fn build_batch(
        &self,
        spans: Vec<TraceSpanRecord>,
    ) -> Result<RecordBatch, TraceEngineError> {
        let start_time = std::time::Instant::now();
        let mut builder = TraceSpanBatchBuilder::new(self.schema.clone());

        for span in spans {
            builder.append(&span)?;
        }

        let record_batch = builder
            .finish()
            .inspect_err(|e| error!("Failed to build RecordBatch: {}", e))?;

        let duration = start_time.elapsed();
        debug!(
            "Built RecordBatch with {} rows in {:?}",
            record_batch.num_rows(),
            duration
        );
        Ok(record_batch)
    }

    /// Build the shared `WriterProperties` used for both ingest writes and Z-ORDER compaction.
    fn build_writer_props() -> WriterProperties {
        WriterProperties::builder()
            // Row group size: creates ~4 groups per 128MB file so bloom + page stats
            // prune within files, not just across files.
            .set_max_row_group_size(32_768)
            // Bloom filter on trace_id: skips ~99% of row groups for trace_id equality lookups.
            .set_column_bloom_filter_enabled(ColumnPath::new(vec!["trace_id".to_string()]), true)
            .set_column_bloom_filter_fpp(ColumnPath::new(vec!["trace_id".to_string()]), 0.01)
            .set_column_bloom_filter_ndv(ColumnPath::new(vec!["trace_id".to_string()]), 32_768)
            // entity_id: high cardinality (UUIDs), very hot equality predicate
            .set_column_bloom_filter_enabled(ColumnPath::new(vec!["entity_id".to_string()]), true)
            .set_column_bloom_filter_fpp(ColumnPath::new(vec!["entity_id".to_string()]), 0.01)
            .set_column_bloom_filter_ndv(ColumnPath::new(vec!["entity_id".to_string()]), 32_768)
            // service_name: low cardinality but hot lookup path — bloom skips row groups fast
            .set_column_bloom_filter_enabled(
                ColumnPath::new(vec!["service_name".to_string()]),
                true,
            )
            .set_column_bloom_filter_fpp(ColumnPath::new(vec!["service_name".to_string()]), 0.01)
            .set_column_bloom_filter_ndv(ColumnPath::new(vec!["service_name".to_string()]), 256)
            // span_name: high cardinality equality queries (e.g. "grpc.unary/method")
            .set_column_bloom_filter_enabled(ColumnPath::new(vec!["span_name".to_string()]), true)
            .set_column_bloom_filter_fpp(ColumnPath::new(vec!["span_name".to_string()]), 0.01)
            .set_column_bloom_filter_ndv(ColumnPath::new(vec!["span_name".to_string()]), 32_768)
            // Page-level stats on start_time: finest-grained time pruning within row groups.
            .set_column_statistics_enabled(
                ColumnPath::new(vec!["start_time".to_string()]),
                EnabledStatistics::Page,
            )
            // status_code: page-level min/max prunes pages for error-only queries.
            // Do NOT use bloom filter: only 3 possible values (0/1/2), overhead > benefit.
            .set_column_statistics_enabled(
                ColumnPath::new(vec!["status_code".to_string()]),
                EnabledStatistics::Page,
            )
            // Delta encoding on near-sorted integer columns: 4-8x compression on timestamps
            // after Z-ORDER compaction; 2-4x on durations within a service.
            .set_column_encoding(
                ColumnPath::new(vec!["start_time".to_string()]),
                Encoding::DELTA_BINARY_PACKED,
            )
            .set_column_encoding(
                ColumnPath::new(vec!["duration_ms".to_string()]),
                Encoding::DELTA_BINARY_PACKED,
            )
            // ZSTD level 3: ~40% better compression than SNAPPY on text columns;
            // marginal decompression overhead is offset by reduced I/O.
            .set_compression(Compression::ZSTD(ZstdLevel::try_new(3).unwrap()))
            // Dictionary hint on span_name: high repetition similar to service_name.
            .set_column_dictionary_enabled(ColumnPath::new(vec!["span_name".to_string()]), true)
            .build()
    }

    /// Write spans to the Delta table (single-writer invariant via actor channel).
    async fn write_spans(&self, spans: Vec<TraceSpanRecord>) -> Result<(), TraceEngineError> {
        info!("Engine received write request for {} spans", spans.len());

        let batch = self
            .build_batch(spans)
            .inspect_err(|e| error!("failed to build batch: {:?}", e))?;
        info!("Built batch with {} rows", batch.num_rows());

        let mut table_guard = self.table.write().await;
        info!("Acquired table write lock");

        // update_incremental is intentionally omitted here.
        //
        // This engine runs as a single-writer actor — no other process commits to this
        // Delta table, so the in-memory state is always current. Calling update_incremental
        // on a freshly-created empty table (version 0, no data files) causes the Delta
        // Kernel to emit "Not a Delta table: No files in log segment", which mutates
        // table_guard into a corrupted intermediate state before the error propagates.
        // That corrupted clone then has no partition column metadata, producing unpartitioned
        // flat Parquet files instead of partition_date=YYYY-MM-DD/ hive directories.

        let current_table = table_guard.clone();

        let updated_table = current_table
            .write(vec![batch])
            .with_save_mode(deltalake::protocol::SaveMode::Append)
            .with_writer_properties(Self::build_writer_props())
            // Always declare partition columns explicitly — do not rely solely on the
            // in-memory snapshot, which can be stale after a failed update_incremental.
            .with_partition_columns(vec!["partition_date".to_string()])
            .await?;

        info!("Successfully wrote batch to Delta Lake");

        self.ctx.deregister_table(TRACE_SPAN_TABLE_NAME)?;
        self.ctx
            .register_table(TRACE_SPAN_TABLE_NAME, updated_table.table_provider().await?)?;

        *table_guard = updated_table;

        Ok(())
    }

    async fn optimize_table(&self) -> Result<(), TraceEngineError> {
        let mut table_guard = self.table.write().await;

        let current_table = table_guard.clone();

        let (updated_table, _metrics) = current_table
            .optimize()
            .with_target_size(128 * 1024 * 1024)
            .with_type(OptimizeType::ZOrder(vec![
                "start_time".to_string(),   // PRIMARY — always present, dominant pruning key
                "entity_id".to_string(),    // SECONDARY — primary scouter query dimension
                "service_name".to_string(), // TERTIARY — service-level filter bonus
            ]))
            // Bloom filters must be re-specified here — compaction rewrites all Parquet files
            // from scratch using these properties. Without this, every compaction cycle
            // silently discards all bloom filters on the rewritten files.
            .with_writer_properties(Self::build_writer_props())
            .await?;

        self.ctx.deregister_table(TRACE_SPAN_TABLE_NAME)?;
        self.ctx
            .register_table(TRACE_SPAN_TABLE_NAME, updated_table.table_provider().await?)?;

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

        self.ctx.deregister_table(TRACE_SPAN_TABLE_NAME)?;
        self.ctx
            .register_table(TRACE_SPAN_TABLE_NAME, updated_table.table_provider().await?)?;

        *table_guard = updated_table;

        Ok(())
    }

    /// Delete all rows with `partition_date` older than `cutoff_date`.
    ///
    /// This is a logical delete — it writes a new Delta log entry marking the rows as removed.
    /// Physical disk space is not reclaimed until `vacuum_table()` runs afterwards.
    async fn expire_table(&self, cutoff_date: chrono::NaiveDate) -> Result<(), TraceEngineError> {
        let mut table_guard = self.table.write().await;

        // CAST('YYYY-MM-DD' AS DATE) produces a Date32 that matches the partition column type,
        // which allows Delta Lake to translate this into a partition directory filter.
        let predicate = format!(
            "partition_date < CAST('{}' AS DATE)",
            cutoff_date.format("%Y-%m-%d")
        );

        let (updated_table, metrics) = table_guard
            .clone()
            .delete()
            .with_predicate(predicate)
            .await?;

        info!(
            "Expired {} rows older than {}",
            metrics.num_deleted_rows, cutoff_date
        );

        self.ctx.deregister_table(TRACE_SPAN_TABLE_NAME)?;
        self.ctx
            .register_table(TRACE_SPAN_TABLE_NAME, updated_table.table_provider().await?)?;

        *table_guard = updated_table;

        Ok(())
    }

    /// Try to claim and run the optimize task via the control table.
    ///
    /// The control table's OCC ensures only one pod runs this at a time across
    /// the entire K8s deployment.
    async fn try_run_optimize(&self, interval_hours: u64) {
        match self.control.try_claim_task(TASK_OPTIMIZE).await {
            Ok(true) => match self.optimize_table().await {
                Ok(()) => {
                    let _ = self
                        .control
                        .release_task(
                            TASK_OPTIMIZE,
                            chrono::Duration::hours(interval_hours as i64),
                        )
                        .await;
                }
                Err(e) => {
                    error!("Optimize failed: {}", e);
                    let _ = self.control.release_task_on_failure(TASK_OPTIMIZE).await;
                }
            },
            Ok(false) => { /* not due or another pod owns it */ }
            Err(e) => error!("Optimize claim check failed: {}", e),
        }
    }

    /// Try to claim and run the retention task via the control table.
    async fn try_run_retention(&self, retention_days: u32) {
        match self.control.try_claim_task(TASK_RETENTION).await {
            Ok(true) => {
                let cutoff =
                    (Utc::now() - chrono::Duration::days(retention_days as i64)).date_naive();
                match self.expire_table(cutoff).await {
                    Ok(()) => {
                        // Reclaim disk space after logical delete
                        let _ = self.vacuum_table(0).await;
                        let _ = self
                            .control
                            .release_task(TASK_RETENTION, chrono::Duration::hours(24))
                            .await;
                    }
                    Err(e) => {
                        error!("Retention failed: {}", e);
                        let _ = self.control.release_task_on_failure(TASK_RETENTION).await;
                    }
                }
            }
            Ok(false) => {}
            Err(e) => error!("Retention claim check failed: {}", e),
        }
    }

    #[instrument(skip_all, name = "trace_engine_actor")]
    pub fn start_actor(
        self,
        compaction_interval_hours: u64,
        retention_days: Option<u32>,
    ) -> (mpsc::Sender<TableCommand>, tokio::task::JoinHandle<()>) {
        let (tx, mut rx) = mpsc::channel::<TableCommand>(100);

        let handle = tokio::spawn(async move {
            // Poll every 5 minutes — the actual schedule is persisted in the
            // control table's `next_run_at` and survives pod restarts.
            let mut scheduler_ticker = interval(Duration::from_secs(5 * 60));
            scheduler_ticker.tick().await; // skip immediate tick

            loop {
                tokio::select! {
                    Some(cmd) = rx.recv() => {
                        match cmd {
                            TableCommand::Write { spans, respond_to } => {
                                match self.write_spans(spans).await {
                                    Ok(_) => { let _ = respond_to.send(Ok(())); }
                                    Err(e) => {
                                        tracing::error!("Write failed: {}", e);
                                        let _ = respond_to.send(Err(e));
                                    }
                                }
                            }
                            TableCommand::Optimize { respond_to } => {
                                // Direct admin request — bypass control table
                                let _ = respond_to.send(self.optimize_table().await);
                            }
                            TableCommand::Vacuum { retention_hours, respond_to } => {
                                let _ = respond_to.send(self.vacuum_table(retention_hours).await);
                            }
                            TableCommand::Expire { cutoff_date, respond_to } => {
                                let _ = respond_to.send(self.expire_table(cutoff_date).await);
                            }
                            TableCommand::Shutdown => {
                                tracing::info!("Shutting down table engine");
                                break;
                            }
                        }
                    }
                    _ = scheduler_ticker.tick() => {
                        self.try_run_optimize(compaction_interval_hours).await;
                        if let Some(days) = retention_days {
                            self.try_run_retention(days).await;
                        }
                    }
                }
            }
        });

        (tx, handle)
    }
}

/// Efficient builder for converting `TraceSpanRecord` (ingest type) into Arrow `RecordBatch`.
///
/// Hierarchy fields (depth, span_order, path, root_span_id) are NOT included — they are
/// computed at query time from the flat span data stored here.
pub struct TraceSpanBatchBuilder {
    schema: SchemaRef,

    // ID builders
    trace_id: FixedSizeBinaryBuilder,
    span_id: FixedSizeBinaryBuilder,
    parent_span_id: FixedSizeBinaryBuilder,

    // W3C Trace Context
    flags: Int32Builder,
    trace_state: StringBuilder,

    // Instrumentation scope
    scope_name: StringBuilder,
    scope_version: StringBuilder,

    // Metadata builders
    service_name: StringDictionaryBuilder<Int32Type>,
    span_name: StringBuilder,
    span_kind: StringDictionaryBuilder<Int8Type>,

    // Time builders
    start_time: TimestampMicrosecondBuilder,
    end_time: TimestampMicrosecondBuilder,
    duration_ms: Int64Builder,

    // Status builders
    status_code: Int32Builder,
    status_message: StringBuilder,

    // Scouter-specific
    label: StringBuilder,
    entity_id: StringBuilder,

    // Attribute builders
    attributes: MapBuilder<StringBuilder, StringViewBuilder>,
    resource_attributes: MapBuilder<StringBuilder, StringViewBuilder>,

    // Nested structure builders
    events: ListBuilder<StructBuilder>,
    links: ListBuilder<StructBuilder>,

    // Payload builders
    input: StringViewBuilder,
    output: StringViewBuilder,

    // Search optimizer
    search_blob: StringViewBuilder,

    // Partition key (days since Unix epoch)
    partition_date: Date32Builder,
}

impl TraceSpanBatchBuilder {
    pub fn new(schema: SchemaRef) -> Self {
        let trace_id = FixedSizeBinaryBuilder::new(16);
        let span_id = FixedSizeBinaryBuilder::new(8);
        let parent_span_id = FixedSizeBinaryBuilder::new(8);

        let flags = Int32Builder::new();
        let trace_state = StringBuilder::new();

        let scope_name = StringBuilder::new();
        let scope_version = StringBuilder::new();

        let service_name = StringDictionaryBuilder::<Int32Type>::new();
        let span_name = StringBuilder::new();
        let span_kind = StringDictionaryBuilder::<Int8Type>::new();

        let start_time = TimestampMicrosecondBuilder::new().with_timezone("UTC");
        let end_time = TimestampMicrosecondBuilder::new().with_timezone("UTC");
        let duration_ms = Int64Builder::new();

        let status_code = Int32Builder::new();
        let status_message = StringBuilder::new();

        let label = StringBuilder::new();
        let entity_id = StringBuilder::new();

        let map_field_name = MapFieldNames {
            entry: "key_value".to_string(),
            key: "key".to_string(),
            value: "value".to_string(),
        };
        let attributes = MapBuilder::new(
            Some(map_field_name.clone()),
            StringBuilder::new(),
            StringViewBuilder::new(),
        );
        let resource_attributes = MapBuilder::new(
            Some(map_field_name.clone()),
            StringBuilder::new(),
            StringViewBuilder::new(),
        );

        let event_fields = vec![
            Field::new("name", DataType::Utf8, false),
            Field::new(
                "timestamp",
                DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
                false,
            ),
            attribute_field(),
            Field::new("dropped_attributes_count", DataType::UInt32, false),
        ];

        let event_struct_builders = vec![
            Box::new(StringBuilder::new()) as Box<dyn ArrayBuilder>,
            Box::new(TimestampMicrosecondBuilder::new().with_timezone("UTC"))
                as Box<dyn ArrayBuilder>,
            Box::new(MapBuilder::new(
                Some(map_field_name.clone()),
                StringBuilder::new(),
                StringViewBuilder::new(),
            )) as Box<dyn ArrayBuilder>,
            Box::new(UInt32Builder::new()) as Box<dyn ArrayBuilder>,
        ];

        let event_struct_builder = StructBuilder::new(event_fields, event_struct_builders);
        let events = ListBuilder::new(event_struct_builder);

        let link_fields = vec![
            Field::new("trace_id", DataType::FixedSizeBinary(16), false),
            Field::new("span_id", DataType::FixedSizeBinary(8), false),
            Field::new("trace_state", DataType::Utf8, false),
            attribute_field(),
            Field::new("dropped_attributes_count", DataType::UInt32, false),
        ];

        let link_struct_builders = vec![
            Box::new(FixedSizeBinaryBuilder::new(16)) as Box<dyn ArrayBuilder>,
            Box::new(FixedSizeBinaryBuilder::new(8)) as Box<dyn ArrayBuilder>,
            Box::new(StringBuilder::new()) as Box<dyn ArrayBuilder>,
            Box::new(MapBuilder::new(
                Some(map_field_name.clone()),
                StringBuilder::new(),
                StringViewBuilder::new(),
            )) as Box<dyn ArrayBuilder>,
            Box::new(UInt32Builder::new()) as Box<dyn ArrayBuilder>,
        ];

        let link_struct_builder = StructBuilder::new(link_fields, link_struct_builders);
        let links = ListBuilder::new(link_struct_builder);

        let input = StringViewBuilder::new();
        let output = StringViewBuilder::new();
        let search_blob = StringViewBuilder::new();
        let partition_date = Date32Builder::new();

        Self {
            schema,
            trace_id,
            span_id,
            parent_span_id,
            flags,
            trace_state,
            scope_name,
            scope_version,
            service_name,
            span_name,
            span_kind,
            start_time,
            end_time,
            duration_ms,
            status_code,
            status_message,
            label,
            entity_id,
            attributes,
            resource_attributes,
            events,
            links,
            input,
            output,
            search_blob,
            partition_date,
        }
    }

    /// Append a single `TraceSpanRecord` to the batch.
    pub fn append(&mut self, span: &TraceSpanRecord) -> Result<(), TraceEngineError> {
        // IDs
        let trace_bytes = span.trace_id.as_bytes();
        self.trace_id
            .append_value(trace_bytes)
            .map_err(TraceEngineError::ArrowError)?;

        let span_bytes = span.span_id.as_bytes();
        self.span_id
            .append_value(span_bytes)
            .map_err(TraceEngineError::ArrowError)?;

        match &span.parent_span_id {
            Some(pid) => {
                self.parent_span_id
                    .append_value(pid.as_bytes())
                    .map_err(TraceEngineError::ArrowError)?;
            }
            None => self.parent_span_id.append_null(),
        }

        // W3C Trace Context
        self.flags.append_value(span.flags);
        self.trace_state.append_value(&span.trace_state);

        // Instrumentation scope
        self.scope_name.append_value(&span.scope_name);
        match &span.scope_version {
            Some(v) => self.scope_version.append_value(v),
            None => self.scope_version.append_null(),
        }

        // Metadata
        self.service_name.append_value(&span.service_name);
        self.span_name.append_value(&span.span_name);
        // span_kind is a non-empty string in TraceSpanRecord — store as non-null
        if span.span_kind.is_empty() {
            self.span_kind.append_null();
        } else {
            self.span_kind.append_value(&span.span_kind);
        }

        // Timestamps
        self.start_time
            .append_value(span.start_time.timestamp_micros());
        self.end_time.append_value(span.end_time.timestamp_micros());
        self.duration_ms.append_value(span.duration_ms);

        // Status
        self.status_code.append_value(span.status_code);
        if span.status_message.is_empty() {
            self.status_message.append_null();
        } else {
            self.status_message.append_value(&span.status_message);
        }

        // Scouter-specific
        match &span.label {
            Some(l) => self.label.append_value(l),
            None => self.label.append_null(),
        }

        // Entity ID — extracted from the scouter.entity span attribute
        let entity_id = span
            .attributes
            .iter()
            .find(|a| a.key == SCOUTER_ENTITY)
            .and_then(|a| a.value.as_str());
        match entity_id {
            Some(uid) => self.entity_id.append_value(uid),
            None => self.entity_id.append_null(),
        }

        // Attributes
        self.append_attributes(&span.attributes).inspect_err(|e| {
            error!(
                "Failed to append attributes for span {}: {}",
                span.span_id, e
            )
        })?;

        // Resource attributes
        self.append_resource_attributes(&span.resource_attributes)
            .inspect_err(|e| {
                error!(
                    "Failed to append resource_attributes for span {}: {}",
                    span.span_id, e
                )
            })?;

        // Events
        self.append_events(&span.events)
            .inspect_err(|e| error!("Failed to append events for span {}: {}", span.span_id, e))?;

        // Links
        self.append_links(&span.links)
            .inspect_err(|e| error!("Failed to append links for span {}: {}", span.span_id, e))?;

        // Payloads
        match &span.input {
            Value::Null => self.input.append_null(),
            v => self.input.append_value(v.to_string()),
        }
        match &span.output {
            Value::Null => self.output.append_null(),
            v => self.output.append_value(v.to_string()),
        }

        // Search blob
        let search_text = Self::build_search_blob(span);
        self.search_blob.append_value(search_text);

        // Partition key — days since Unix epoch, derived from span start date
        let days = span.start_time.date_naive().num_days_from_ce() - UNIX_EPOCH_DAYS;
        self.partition_date.append_value(days);

        Ok(())
    }

    fn append_attributes(&mut self, attributes: &[Attribute]) -> Result<(), TraceEngineError> {
        for attr in attributes {
            self.attributes.keys().append_value(&attr.key);
            let value_str = match &attr.value {
                Value::String(s) => s.clone(),
                Value::Null => String::new(),
                other => other.to_string(),
            };
            self.attributes.values().append_value(value_str);
        }
        self.attributes.append(true)?;
        Ok(())
    }

    fn append_resource_attributes(
        &mut self,
        attributes: &[Attribute],
    ) -> Result<(), TraceEngineError> {
        if attributes.is_empty() {
            self.resource_attributes.append(false)?; // null map
        } else {
            for attr in attributes {
                self.resource_attributes.keys().append_value(&attr.key);
                let value_str = match &attr.value {
                    Value::String(s) => s.clone(),
                    Value::Null => String::new(),
                    other => other.to_string(),
                };
                self.resource_attributes.values().append_value(value_str);
            }
            self.resource_attributes.append(true)?;
        }
        Ok(())
    }

    fn append_events(&mut self, events: &[SpanEvent]) -> Result<(), TraceEngineError> {
        let event_struct = self.events.values();
        for event in events {
            let name_builder = event_struct
                .field_builder::<StringBuilder>(0)
                .ok_or_else(|| TraceEngineError::DowncastError("event name builder"))?;
            name_builder.append_value(&event.name);

            let time_builder = event_struct
                .field_builder::<TimestampMicrosecondBuilder>(1)
                .ok_or_else(|| TraceEngineError::DowncastError("event timestamp builder"))?;
            time_builder.append_value(event.timestamp.timestamp_micros());

            let attr_builder = event_struct
                .field_builder::<MapBuilder<StringBuilder, StringViewBuilder>>(2)
                .ok_or_else(|| TraceEngineError::DowncastError("event attributes builder"))?;

            for attr in &event.attributes {
                attr_builder.keys().append_value(&attr.key);
                let value_str = match &attr.value {
                    Value::String(s) => s.clone(),
                    Value::Null => String::new(),
                    other => other.to_string(),
                };
                attr_builder.values().append_value(value_str);
            }
            attr_builder.append(true)?;

            let dropped_builder =
                event_struct
                    .field_builder::<UInt32Builder>(3)
                    .ok_or_else(|| {
                        TraceEngineError::DowncastError("dropped attributes count builder")
                    })?;
            dropped_builder.append_value(event.dropped_attributes_count);

            event_struct.append(true);
        }

        self.events.append(true);
        Ok(())
    }

    fn append_links(&mut self, links: &[SpanLink]) -> Result<(), TraceEngineError> {
        let link_struct = self.links.values();

        for link in links {
            let trace_builder = link_struct
                .field_builder::<FixedSizeBinaryBuilder>(0)
                .ok_or_else(|| TraceEngineError::DowncastError("link trace_id builder"))?;

            let trace_bytes = TraceId::hex_to_bytes(&link.trace_id).map_err(|e| {
                TraceEngineError::InvalidHexId(link.trace_id.clone(), e.to_string())
            })?;
            trace_builder.append_value(&trace_bytes)?;

            let span_builder = link_struct
                .field_builder::<FixedSizeBinaryBuilder>(1)
                .ok_or_else(|| TraceEngineError::DowncastError("link span_id builder"))?;

            let span_bytes = SpanId::hex_to_bytes(&link.span_id)
                .map_err(|e| TraceEngineError::InvalidHexId(link.span_id.clone(), e.to_string()))?;
            span_builder.append_value(&span_bytes)?;

            let state_builder = link_struct
                .field_builder::<StringBuilder>(2)
                .ok_or_else(|| TraceEngineError::DowncastError("link trace_state builder"))?;
            state_builder.append_value(&link.trace_state);

            let attr_builder = link_struct
                .field_builder::<MapBuilder<StringBuilder, StringViewBuilder>>(3)
                .ok_or_else(|| TraceEngineError::DowncastError("link attributes builder"))?;

            for attr in &link.attributes {
                attr_builder.keys().append_value(&attr.key);
                let value_str = match &attr.value {
                    Value::String(s) => s.clone(),
                    Value::Null => String::new(),
                    other => other.to_string(),
                };
                attr_builder.values().append_value(value_str);
            }
            attr_builder.append(true)?;

            let dropped_builder =
                link_struct
                    .field_builder::<UInt32Builder>(4)
                    .ok_or_else(|| {
                        TraceEngineError::DowncastError("link dropped attributes count builder")
                    })?;
            dropped_builder.append_value(link.dropped_attributes_count);

            link_struct.append(true);
        }

        self.links.append(true);
        Ok(())
    }

    /// Build a concatenated search string from `TraceSpanRecord` for full-text queries.
    ///
    /// Uses pipe-bounded tokens (`|key=value|`) to prevent false-positive substring matches
    /// where a value contains something that looks like a different attribute key or value.
    /// Queries use `%key=value%` patterns which match both old `key:value` archive data
    /// and the new `|key=value|` format.
    fn build_search_blob(span: &TraceSpanRecord) -> String {
        let mut search = String::with_capacity(512);

        // Pipe-bounded bare tokens for full-text (service, span, scope)
        search.push('|');
        search.push_str(&span.service_name);
        search.push_str("| |");
        search.push_str(&span.span_name);
        search.push_str("| |");
        search.push_str(&span.scope_name);
        search.push('|');

        if !span.status_message.is_empty() {
            search.push_str(" |");
            search.push_str(&span.status_message);
            search.push('|');
        }

        // Pipe-bounded key=value tokens — standardize on `=` separator
        for attr in &span.attributes {
            search.push_str(" |");
            search.push_str(&attr.key);
            search.push('=');
            match &attr.value {
                Value::String(s) => search.push_str(s),
                Value::Number(n) => search.push_str(&n.to_string()),
                Value::Bool(b) => search.push_str(&b.to_string()),
                Value::Null => {}
                other => search.push_str(&other.to_string()),
            }
            search.push('|');
        }

        for event in &span.events {
            search.push_str(" |");
            search.push_str(&event.name);
            search.push('|');
        }

        search
    }

    /// Finalize and build the RecordBatch. Column order must match `create_schema()`.
    pub fn finish(mut self) -> Result<RecordBatch, TraceEngineError> {
        let batch = RecordBatch::try_new(
            self.schema.clone(),
            vec![
                Arc::new(self.trace_id.finish()),
                Arc::new(self.span_id.finish()),
                Arc::new(self.parent_span_id.finish()),
                Arc::new(self.flags.finish()),
                Arc::new(self.trace_state.finish()),
                Arc::new(self.scope_name.finish()),
                Arc::new(self.scope_version.finish()),
                Arc::new(self.service_name.finish()),
                Arc::new(self.span_name.finish()),
                Arc::new(self.span_kind.finish()),
                Arc::new(self.start_time.finish()),
                Arc::new(self.end_time.finish()),
                Arc::new(self.duration_ms.finish()),
                Arc::new(self.status_code.finish()),
                Arc::new(self.status_message.finish()),
                Arc::new(self.label.finish()),
                Arc::new(self.entity_id.finish()),
                Arc::new(self.attributes.finish()),
                Arc::new(self.resource_attributes.finish()),
                Arc::new(self.events.finish()),
                Arc::new(self.links.finish()),
                Arc::new(self.input.finish()),
                Arc::new(self.output.finish()),
                Arc::new(self.search_blob.finish()),
                Arc::new(self.partition_date.finish()),
            ],
        )?;

        Ok(batch)
    }
}
