use crate::error::DatasetEngineError;
use crate::parquet::bifrost::catalog::DatasetCatalogProvider;
use crate::parquet::tracing::traits::arrow_schema_to_delta;
use crate::parquet::utils::register_cloud_logstore_factories;
use crate::storage::ObjectStore;
use arrow::datatypes::{DataType, Schema, SchemaRef};
use arrow_array::RecordBatch;
use datafusion::prelude::SessionContext;
use deltalake::datafusion::parquet::basic::{Compression, Encoding, ZstdLevel};
use deltalake::datafusion::parquet::file::properties::{EnabledStatistics, WriterProperties};
use deltalake::datafusion::parquet::schema::types::ColumnPath;
use deltalake::operations::optimize::OptimizeType;
use deltalake::{DeltaTable, DeltaTableBuilder, TableProperty};
use scouter_types::dataset::schema::{
    SCOUTER_BATCH_ID, SCOUTER_CREATED_AT, SCOUTER_PARTITION_DATE,
};
use scouter_types::dataset::DatasetNamespace;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio::sync::{mpsc, RwLock as AsyncRwLock};
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, instrument};
use url::Url;

const MIN_VACUUM_RETENTION_HOURS: u64 = 1;

pub enum TableCommand {
    Write {
        batches: Vec<RecordBatch>,
        respond_to: oneshot::Sender<Result<(), DatasetEngineError>>,
    },
    Optimize {
        respond_to: oneshot::Sender<Result<(), DatasetEngineError>>,
    },
    Vacuum {
        retention_hours: u64,
        respond_to: oneshot::Sender<Result<(), DatasetEngineError>>,
    },
    Shutdown,
}

fn build_table_url(
    object_store: &ObjectStore,
    namespace: &DatasetNamespace,
) -> Result<Url, DatasetEngineError> {
    let mut base = object_store.get_base_url()?;
    let mut path = base.path().to_string();
    if !path.ends_with('/') {
        path.push('/');
    }
    path.push_str(&namespace.storage_path());
    base.set_path(&path);
    Ok(base)
}

/// Attempt to load an existing Delta table or create a new one.
#[instrument(skip_all, fields(namespace = %namespace.fqn()))]
async fn build_or_create_table(
    object_store: &ObjectStore,
    schema: &Schema,
    namespace: &DatasetNamespace,
    partition_columns: &[String],
) -> Result<DeltaTable, DatasetEngineError> {
    register_cloud_logstore_factories();
    let table_url = build_table_url(object_store, namespace)?;
    info!(
        "Attempting to load dataset table [{}://.../{} ]",
        table_url.scheme(),
        namespace.fqn()
    );

    // For local filesystem, ensure the directory exists
    if table_url.scheme() == "file" {
        if let Ok(path) = table_url.to_file_path() {
            if !path.exists() {
                info!("Creating directory for local table: {:?}", path);
                std::fs::create_dir_all(&path)?;
            }
        }
    }

    // Try a single load attempt
    let store = object_store.as_dyn_object_store();
    let load_result = DeltaTableBuilder::from_url(table_url.clone())
        .map(|builder| builder.with_storage_backend(store, table_url.clone()));

    if let Ok(builder) = load_result {
        if let Ok(table) = builder.load().await {
            info!("Loaded existing dataset table [{}]", namespace.fqn());
            return Ok(table);
        }
    }

    // Table doesn't exist yet — create it
    info!("Creating new dataset table [{}]", namespace.fqn());
    let store = object_store.as_dyn_object_store();
    let table = DeltaTableBuilder::from_url(table_url.clone())?
        .with_storage_backend(store, table_url)
        .build()?;

    let delta_fields = arrow_schema_to_delta(schema);

    let data_skipping_cols = build_data_skipping_columns(partition_columns);

    let table = table
        .create()
        .with_table_name(namespace.fqn())
        .with_columns(delta_fields)
        .with_partition_columns(partition_columns.to_vec())
        .with_configuration_property(TableProperty::CheckpointInterval, Some("5"))
        .with_configuration_property(
            TableProperty::DataSkippingStatsColumns,
            Some(&data_skipping_cols),
        )
        .await?;

    Ok(table)
}

fn build_data_skipping_columns(partition_columns: &[String]) -> String {
    let mut cols = vec![
        "scouter_created_at".to_string(),
        SCOUTER_PARTITION_DATE.to_string(),
    ];
    for col in partition_columns {
        if !cols.contains(col) {
            cols.push(col.clone());
        }
    }
    cols.join(",")
}

/// Build Parquet writer properties for a dataset with a dynamic schema.
///
/// System columns get hardcoded optimizations. User columns ending in `_id`
/// or `_key` (Utf8/Utf8View) get bloom filters automatically.
pub fn build_writer_props(schema: &Schema) -> WriterProperties {
    let mut builder = WriterProperties::builder()
        .set_max_row_group_size(32_768)
        .set_compression(Compression::ZSTD(ZstdLevel::try_new(3).unwrap()))
        .set_column_encoding(
            ColumnPath::new(vec![SCOUTER_CREATED_AT.to_string()]),
            Encoding::DELTA_BINARY_PACKED,
        )
        .set_column_bloom_filter_enabled(ColumnPath::new(vec![SCOUTER_BATCH_ID.to_string()]), true)
        .set_column_bloom_filter_fpp(ColumnPath::new(vec![SCOUTER_BATCH_ID.to_string()]), 0.01)
        .set_column_bloom_filter_ndv(ColumnPath::new(vec![SCOUTER_BATCH_ID.to_string()]), 10_000)
        .set_column_statistics_enabled(
            ColumnPath::new(vec![SCOUTER_CREATED_AT.to_string()]),
            EnabledStatistics::Page,
        );

    for field in schema.fields() {
        let name = field.name();
        if (name.ends_with("_id") || name.ends_with("_key"))
            && matches!(
                field.data_type(),
                DataType::Utf8 | DataType::Utf8View | DataType::LargeUtf8
            )
            && name != SCOUTER_BATCH_ID
        {
            builder = builder
                .set_column_bloom_filter_enabled(ColumnPath::new(vec![name.clone()]), true)
                .set_column_bloom_filter_fpp(ColumnPath::new(vec![name.clone()]), 0.01)
                .set_column_bloom_filter_ndv(ColumnPath::new(vec![name.clone()]), 10_000);
        }
    }

    builder.build()
}

/// Per-table dataset engine actor.
///
/// Owns a single `DeltaTable` and serializes all writes through an mpsc channel
/// (single-writer invariant). Follows the `TraceSpanDBEngine` actor pattern.
pub struct DatasetEngine {
    schema: SchemaRef,
    _object_store: ObjectStore,
    table: Arc<AsyncRwLock<DeltaTable>>,
    write_ctx: Arc<SessionContext>,
    namespace: DatasetNamespace,
    partition_columns: Vec<String>,
    catalog_provider: Arc<DatasetCatalogProvider>,
}

impl DatasetEngine {
    pub async fn new(
        object_store: &ObjectStore,
        schema: SchemaRef,
        namespace: DatasetNamespace,
        partition_columns: Vec<String>,
        catalog_provider: Arc<DatasetCatalogProvider>,
    ) -> Result<Self, DatasetEngineError> {
        let delta_table =
            build_or_create_table(object_store, &schema, &namespace, &partition_columns).await?;
        let write_ctx = object_store.get_session()?;

        // Register table in write context with a simple name (no catalog resolution needed).
        // The write_ctx is private — only used for the deregister/register cycle after writes.
        let write_table_name = Self::write_table_name(&namespace);
        if let Ok(provider) = delta_table.table_provider().await {
            write_ctx.register_table(&write_table_name, provider)?;
        } else {
            info!(
                "Empty table at init — deferring write_ctx registration until first write [{}]",
                namespace.fqn()
            );
        }

        // Register table in the shared catalog for query access
        if let Ok(provider) = delta_table.table_provider().await {
            catalog_provider.swap_table(&namespace, provider);
        }

        Ok(Self {
            schema,
            _object_store: object_store.clone(),
            table: Arc::new(AsyncRwLock::new(delta_table)),
            write_ctx: Arc::new(write_ctx),
            namespace,
            partition_columns,
            catalog_provider,
        })
    }

    /// Simple table name for the private write_ctx (avoids catalog resolution).
    fn write_table_name(namespace: &DatasetNamespace) -> String {
        format!(
            "_write_{}_{}_{}",
            namespace.catalog, namespace.schema_name, namespace.table
        )
    }

    async fn write_batches(&self, batches: Vec<RecordBatch>) -> Result<(), DatasetEngineError> {
        let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
        info!(
            "Engine writing {} batches ({} rows) to [{}]",
            batches.len(),
            total_rows,
            self.namespace.fqn()
        );

        let mut table_guard = self.table.write().await;

        // Clone before mutation — preserves original state on error
        let current_table = table_guard.clone();

        let updated_table = current_table
            .write(batches)
            .with_save_mode(deltalake::protocol::SaveMode::Append)
            .with_writer_properties(build_writer_props(&self.schema))
            .with_partition_columns(self.partition_columns.clone())
            .await?;

        // Compute the new provider once — reused for both write_ctx and catalog swap.
        // This avoids a second async call and ensures the write_ctx update cannot
        // leave the context in a torn state if table_provider() fails.
        let new_provider = updated_table.table_provider().await?;

        // Update private write context
        let write_name = Self::write_table_name(&self.namespace);
        let _ = self.write_ctx.deregister_table(&write_name);
        self.write_ctx
            .register_table(&write_name, Arc::clone(&new_provider))?;
        updated_table.update_datafusion_session(&self.write_ctx.state())?;

        // Update shared catalog — atomic TableProvider swap
        self.catalog_provider
            .swap_table(&self.namespace, new_provider);

        *table_guard = updated_table;

        debug!(
            "Successfully wrote {} rows to [{}]",
            total_rows,
            self.namespace.fqn()
        );
        Ok(())
    }

    async fn optimize_table(&self) -> Result<(), DatasetEngineError> {
        info!("Optimizing dataset table [{}]", self.namespace.fqn());
        let mut table_guard = self.table.write().await;

        let current_table = table_guard.clone();

        let mut z_order_cols = vec!["scouter_created_at".to_string()];
        // Add first user partition column (if any beyond scouter_partition_date)
        for col in &self.partition_columns {
            if col != "scouter_partition_date" {
                z_order_cols.push(col.clone());
                break;
            }
        }

        let (updated_table, _metrics) = current_table
            .optimize()
            .with_target_size(std::num::NonZero::new(128 * 1024 * 1024).unwrap())
            .with_type(OptimizeType::ZOrder(z_order_cols))
            .with_writer_properties(build_writer_props(&self.schema))
            .await?;

        let write_name = Self::write_table_name(&self.namespace);
        let _ = self.write_ctx.deregister_table(&write_name);
        self.write_ctx
            .register_table(&write_name, updated_table.table_provider().await?)?;
        updated_table.update_datafusion_session(&self.write_ctx.state())?;

        let provider = updated_table.table_provider().await?;
        self.catalog_provider.swap_table(&self.namespace, provider);

        *table_guard = updated_table;

        info!("Optimization complete for [{}]", self.namespace.fqn());
        Ok(())
    }

    async fn vacuum_table(&self, retention_hours: u64) -> Result<(), DatasetEngineError> {
        let retention_hours = retention_hours.max(MIN_VACUUM_RETENTION_HOURS);
        info!(
            "Vacuuming dataset table [{}] (retention: {}h)",
            self.namespace.fqn(),
            retention_hours
        );
        let mut table_guard = self.table.write().await;

        let (updated_table, _metrics) = table_guard
            .clone()
            .vacuum()
            .with_retention_period(chrono::Duration::hours(retention_hours as i64))
            .with_enforce_retention_duration(false)
            .await?;

        let write_name = Self::write_table_name(&self.namespace);
        let _ = self.write_ctx.deregister_table(&write_name);
        self.write_ctx
            .register_table(&write_name, updated_table.table_provider().await?)?;
        updated_table.update_datafusion_session(&self.write_ctx.state())?;

        let provider = updated_table.table_provider().await?;
        self.catalog_provider.swap_table(&self.namespace, provider);

        *table_guard = updated_table;

        info!(
            "Vacuum complete for [{}] (retention: {}h)",
            self.namespace.fqn(),
            retention_hours
        );
        Ok(())
    }

    async fn refresh_table(&self) -> Result<(), DatasetEngineError> {
        let mut table_guard = self.table.write().await;
        let current_version = table_guard.version();
        let mut refreshed = table_guard.clone();

        match refreshed.update_incremental(None).await {
            Ok(_) => {
                if refreshed.version() > current_version {
                    debug!(
                        "Refreshed [{}]: v{:?} → v{:?}",
                        self.namespace.fqn(),
                        current_version,
                        refreshed.version()
                    );

                    // Compute provider first — only deregister/re-register if it succeeds,
                    // so the write_ctx is never left in a torn (no table registered) state.
                    if let Ok(new_provider) = refreshed.table_provider().await {
                        let write_name = Self::write_table_name(&self.namespace);
                        let _ = self.write_ctx.deregister_table(&write_name);
                        self.write_ctx
                            .register_table(&write_name, Arc::clone(&new_provider))?;
                        refreshed.update_datafusion_session(&self.write_ctx.state())?;
                        self.catalog_provider
                            .swap_table(&self.namespace, new_provider);
                        *table_guard = refreshed;
                    }
                }
            }
            Err(e) => {
                debug!("Refresh skipped for [{}]: {}", self.namespace.fqn(), e);
            }
        }

        Ok(())
    }

    /// Start the actor loop. Returns the command channel sender and join handle.
    #[instrument(skip_all, name = "dataset_engine_actor", fields(fqn = %self.namespace.fqn()))]
    pub fn start_actor(
        self,
        refresh_interval_secs: u64,
    ) -> (mpsc::Sender<TableCommand>, tokio::task::JoinHandle<()>) {
        let (tx, mut rx) = mpsc::channel::<TableCommand>(50);

        let handle = tokio::spawn(async move {
            // Clamp to 1s minimum — tokio::time::interval panics on Duration::ZERO.
            let mut refresh_ticker = interval(Duration::from_secs(refresh_interval_secs.max(1)));
            refresh_ticker.tick().await; // skip immediate

            loop {
                tokio::select! {
                    Some(cmd) = rx.recv() => {
                        match cmd {
                            TableCommand::Write { batches, respond_to } => {
                                let result = self.write_batches(batches).await;
                                if let Err(ref e) = result {
                                    error!("Write failed for [{}]: {}", self.namespace.fqn(), e);
                                }
                                let _ = respond_to.send(result);
                            }
                            TableCommand::Optimize { respond_to } => {
                                let _ = respond_to.send(self.optimize_table().await);
                                if let Err(e) = self.vacuum_table(MIN_VACUUM_RETENTION_HOURS).await {
                                    error!("Post-optimize vacuum failed for [{}]: {}", self.namespace.fqn(), e);
                                }
                            }
                            TableCommand::Vacuum { retention_hours, respond_to } => {
                                let _ = respond_to.send(self.vacuum_table(retention_hours).await);
                            }
                            TableCommand::Shutdown => {
                                info!("Shutting down dataset engine [{}]", self.namespace.fqn());
                                break;
                            }
                        }
                    }
                    _ = refresh_ticker.tick() => {
                        if let Err(e) = self.refresh_table().await {
                            error!("Table refresh failed for [{}]: {}", self.namespace.fqn(), e);
                        }
                    }
                }
            }
        });

        (tx, handle)
    }
}
