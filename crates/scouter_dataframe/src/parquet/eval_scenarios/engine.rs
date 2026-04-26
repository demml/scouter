use crate::error::EvalScenarioEngineError;
use crate::parquet::control::{get_pod_id, ControlTableEngine};
use crate::parquet::tracing::catalog::TraceCatalogProvider;
use crate::parquet::tracing::traits::arrow_schema_to_delta;
use crate::parquet::utils::register_cloud_logstore_factories;
use crate::storage::ObjectStore;
use arrow::array::{LargeStringBuilder, StringBuilder, TimestampMicrosecondBuilder};
use arrow::datatypes::{DataType, Field, Schema, SchemaRef, TimeUnit};
use arrow_array::RecordBatch;
use chrono::{DateTime, Utc};
use datafusion::catalog::CatalogProvider;
use datafusion::prelude::SessionContext;
use deltalake::datafusion::parquet::basic::{Compression, ZstdLevel};
use deltalake::datafusion::parquet::file::properties::WriterProperties;
use deltalake::datafusion::parquet::schema::types::ColumnPath;
use deltalake::operations::optimize::OptimizeType;
use deltalake::protocol::SaveMode;
use deltalake::{DeltaTable, DeltaTableBuilder, TableProperty};
use scouter_settings::ObjectStorageSettings;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock as AsyncRwLock};
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, instrument};
use url::Url;

pub const EVAL_SCENARIO_TABLE_NAME: &str = "eval_scenarios";
pub const EVAL_SCENARIO_CATALOG_NAME: &str = "scouter_eval_scenarios";
const EVAL_SCENARIO_DEFAULT_SCHEMA: &str = "default";

const TASK_OPTIMIZE: &str = "eval_scenario_optimize";

// Column names
pub const COLLECTION_ID_COL: &str = "collection_id";
pub const SCENARIO_ID_COL: &str = "scenario_id";
pub const SCENARIO_JSON_COL: &str = "scenario_json";
pub const CREATED_AT_COL: &str = "created_at";

/// Ingest record — one row per `EvalScenario`.
#[derive(Debug, Clone)]
pub struct EvalScenarioRecord {
    pub collection_id: String,
    pub scenario_id: String,
    pub scenario_json: String,
    pub created_at: DateTime<Utc>,
}

pub enum TableCommand {
    Write {
        records: Vec<EvalScenarioRecord>,
        respond_to: oneshot::Sender<Result<(), EvalScenarioEngineError>>,
    },
    Shutdown,
}

fn create_schema() -> Schema {
    Schema::new(vec![
        Field::new(COLLECTION_ID_COL, DataType::Utf8, false),
        Field::new(SCENARIO_ID_COL, DataType::Utf8, false),
        Field::new(SCENARIO_JSON_COL, DataType::LargeUtf8, false),
        Field::new(
            CREATED_AT_COL,
            DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
            false,
        ),
    ])
}

struct EvalScenarioBatchBuilder {
    schema: SchemaRef,
    collection_id: StringBuilder,
    scenario_id: StringBuilder,
    scenario_json: LargeStringBuilder,
    created_at: TimestampMicrosecondBuilder,
}

impl EvalScenarioBatchBuilder {
    fn new(schema: SchemaRef) -> Self {
        Self {
            schema,
            collection_id: StringBuilder::new(),
            scenario_id: StringBuilder::new(),
            scenario_json: LargeStringBuilder::new(),
            created_at: TimestampMicrosecondBuilder::new().with_timezone("UTC".to_string()),
        }
    }

    fn append(&mut self, record: &EvalScenarioRecord) {
        self.collection_id.append_value(&record.collection_id);
        self.scenario_id.append_value(&record.scenario_id);
        self.scenario_json.append_value(&record.scenario_json);
        self.created_at
            .append_value(record.created_at.timestamp_micros());
    }

    fn finish(mut self) -> Result<RecordBatch, EvalScenarioEngineError> {
        Ok(RecordBatch::try_new(
            self.schema.clone(),
            vec![
                Arc::new(self.collection_id.finish()),
                Arc::new(self.scenario_id.finish()),
                Arc::new(self.scenario_json.finish()),
                Arc::new(self.created_at.finish()),
            ],
        )?)
    }
}

fn build_writer_props() -> WriterProperties {
    WriterProperties::builder()
        .set_max_row_group_row_count(Some(32_768))
        .set_column_bloom_filter_enabled(ColumnPath::new(vec![COLLECTION_ID_COL.to_string()]), true)
        .set_column_bloom_filter_fpp(ColumnPath::new(vec![COLLECTION_ID_COL.to_string()]), 0.01)
        .set_column_bloom_filter_ndv(ColumnPath::new(vec![COLLECTION_ID_COL.to_string()]), 10_000)
        .set_compression(Compression::ZSTD(ZstdLevel::try_new(3).unwrap()))
        .build()
}

async fn build_url(object_store: &ObjectStore) -> Result<Url, EvalScenarioEngineError> {
    let mut base = object_store.get_base_url()?;
    let mut path = base.path().to_string();
    if !path.ends_with('/') {
        path.push('/');
    }
    path.push_str(EVAL_SCENARIO_TABLE_NAME);
    base.set_path(&path);
    Ok(base)
}

#[instrument(skip_all)]
async fn create_table(
    object_store: &ObjectStore,
    table_url: Url,
    schema: SchemaRef,
) -> Result<DeltaTable, EvalScenarioEngineError> {
    info!(
        "Creating eval scenario table [{}://.../{} ]",
        table_url.scheme(),
        table_url
            .path_segments()
            .and_then(|mut s| s.next_back())
            .unwrap_or(EVAL_SCENARIO_TABLE_NAME)
    );

    let store = object_store.as_dyn_object_store();
    let table = DeltaTableBuilder::from_url(table_url.clone())?
        .with_storage_backend(store, table_url)
        .build()?;

    let delta_fields = arrow_schema_to_delta(&schema);
    table
        .create()
        .with_table_name(EVAL_SCENARIO_TABLE_NAME)
        .with_columns(delta_fields)
        .with_configuration_property(TableProperty::CheckpointInterval, Some("5"))
        .with_configuration_property(
            TableProperty::DataSkippingStatsColumns,
            Some("collection_id,created_at"),
        )
        .await
        .map_err(Into::into)
}

#[instrument(skip_all)]
async fn build_or_create_table(
    object_store: &ObjectStore,
    schema: SchemaRef,
) -> Result<DeltaTable, EvalScenarioEngineError> {
    register_cloud_logstore_factories();
    let table_url = build_url(object_store).await?;
    info!(
        "Attempting to load eval scenario table [{}://.../{} ]",
        table_url.scheme(),
        table_url
            .path_segments()
            .and_then(|mut s| s.next_back())
            .unwrap_or(EVAL_SCENARIO_TABLE_NAME)
    );

    let is_delta_table = if table_url.scheme() == "file" {
        if let Ok(path) = table_url.to_file_path() {
            if !path.exists() {
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
        let store = object_store.as_dyn_object_store();
        let table = DeltaTableBuilder::from_url(table_url.clone())?
            .with_storage_backend(store, table_url)
            .load()
            .await?;
        Ok(table)
    } else {
        create_table(object_store, table_url, schema).await
    }
}

pub struct EvalScenarioDBEngine {
    schema: SchemaRef,
    pub object_store: ObjectStore,
    table: Arc<AsyncRwLock<DeltaTable>>,
    ctx: Arc<SessionContext>,
    catalog: Arc<TraceCatalogProvider>,
    control: ControlTableEngine,
}

impl EvalScenarioDBEngine {
    pub async fn new(
        storage_settings: &ObjectStorageSettings,
    ) -> Result<Self, EvalScenarioEngineError> {
        let object_store = ObjectStore::new(storage_settings)?;
        let schema = Arc::new(create_schema());
        let delta_table = build_or_create_table(&object_store, schema.clone()).await?;

        let ctx = object_store
            .get_session_with_catalog(EVAL_SCENARIO_CATALOG_NAME, EVAL_SCENARIO_DEFAULT_SCHEMA)?;

        let catalog = Arc::new(TraceCatalogProvider::new());
        ctx.register_catalog(
            EVAL_SCENARIO_CATALOG_NAME,
            Arc::clone(&catalog) as Arc<dyn CatalogProvider>,
        );

        if let Ok(provider) = delta_table.table_provider().await {
            catalog.swap(EVAL_SCENARIO_TABLE_NAME, provider);
        } else {
            info!("Empty eval scenario table — deferring catalog registration until first write");
        }

        let control = ControlTableEngine::new(&object_store, get_pod_id())
            .await
            .map_err(|e| EvalScenarioEngineError::EngineError(e.to_string()))?;

        Ok(EvalScenarioDBEngine {
            schema,
            object_store,
            table: Arc::new(AsyncRwLock::new(delta_table)),
            ctx: Arc::new(ctx),
            catalog,
            control,
        })
    }

    pub fn ctx(&self) -> Arc<SessionContext> {
        Arc::clone(&self.ctx)
    }

    fn build_batch(
        &self,
        records: Vec<EvalScenarioRecord>,
    ) -> Result<RecordBatch, EvalScenarioEngineError> {
        let mut builder = EvalScenarioBatchBuilder::new(self.schema.clone());
        for record in &records {
            builder.append(record);
        }
        let batch = builder
            .finish()
            .inspect_err(|e| error!("Failed to build RecordBatch: {}", e))?;
        debug!("Built RecordBatch with {} rows", batch.num_rows());
        Ok(batch)
    }

    async fn write_records(
        &self,
        records: Vec<EvalScenarioRecord>,
    ) -> Result<(), EvalScenarioEngineError> {
        info!("Writing {} eval scenario records", records.len());

        let batch = self.build_batch(records)?;
        let mut table_guard = self.table.write().await;
        let current_table = table_guard.clone();

        let updated_table = current_table
            .write(vec![batch])
            .with_save_mode(SaveMode::Append)
            .with_writer_properties(build_writer_props())
            .await?;

        let new_provider = updated_table.table_provider().await?;
        self.catalog.swap(EVAL_SCENARIO_TABLE_NAME, new_provider);
        updated_table.update_datafusion_session(&self.ctx.state())?;

        *table_guard = updated_table;

        info!("Eval scenario records written successfully");
        Ok(())
    }

    async fn optimize_table(&self) -> Result<(), EvalScenarioEngineError> {
        let mut table_guard = self.table.write().await;

        let current_table = table_guard.clone();

        let (updated_table, _metrics) = current_table
            .optimize()
            .with_target_size(std::num::NonZero::new(128 * 1024 * 1024).unwrap())
            .with_type(OptimizeType::ZOrder(vec![COLLECTION_ID_COL.to_string()]))
            // Bloom filters must be re-specified — compaction rewrites all Parquet files
            // from scratch, silently discarding existing bloom filters without this.
            .with_writer_properties(build_writer_props())
            .await?;

        self.catalog.swap(
            EVAL_SCENARIO_TABLE_NAME,
            updated_table.table_provider().await?,
        );
        updated_table.update_datafusion_session(&self.ctx.state())?;
        *table_guard = updated_table;

        Ok(())
    }

    async fn vacuum_table(&self) -> Result<(), EvalScenarioEngineError> {
        let mut table_guard = self.table.write().await;

        let (updated_table, _metrics) = table_guard
            .clone()
            .vacuum()
            .with_retention_period(chrono::Duration::days(7))
            .with_enforce_retention_duration(false)
            .await?;

        self.catalog.swap(
            EVAL_SCENARIO_TABLE_NAME,
            updated_table.table_provider().await?,
        );
        updated_table.update_datafusion_session(&self.ctx.state())?;
        *table_guard = updated_table;

        Ok(())
    }

    async fn try_run_optimize(&self, interval_hours: u64) {
        match self.control.try_claim_task(TASK_OPTIMIZE).await {
            Ok(true) => match self.optimize_table().await {
                Ok(()) => {
                    if let Err(e) = self.vacuum_table().await {
                        error!("Post-optimize vacuum failed: {}", e);
                    }
                    let _ = self
                        .control
                        .release_task(
                            TASK_OPTIMIZE,
                            chrono::Duration::hours(interval_hours as i64),
                        )
                        .await;
                }
                Err(e) => {
                    error!("Eval scenario optimize failed: {}", e);
                    let _ = self.control.release_task_on_failure(TASK_OPTIMIZE).await;
                }
            },
            Ok(false) => {}
            Err(e) => error!("Eval scenario optimize claim check failed: {}", e),
        }
    }

    #[instrument(skip_all, name = "eval_scenario_engine_actor")]
    pub fn start_actor(
        self,
        compaction_interval_hours: u64,
    ) -> (mpsc::Sender<TableCommand>, tokio::task::JoinHandle<()>) {
        let (tx, mut rx) = mpsc::channel::<TableCommand>(32);

        let handle = tokio::spawn(async move {
            info!("EvalScenarioDBEngine actor started");

            // Poll every 5 minutes — actual schedule is persisted in the control
            // table's `next_run_at` and survives pod restarts.
            let mut scheduler_ticker = interval(Duration::from_secs(5 * 60));
            scheduler_ticker.tick().await; // skip immediate tick

            loop {
                tokio::select! {
                    cmd = rx.recv() => {
                        match cmd {
                            Some(TableCommand::Write { records, respond_to }) => {
                                let result = self.write_records(records).await;
                                if result.is_err() {
                                    error!("Write failed: {:?}", result);
                                }
                                let _ = respond_to.send(result);
                            }
                            Some(TableCommand::Shutdown) | None => {
                                info!("EvalScenarioDBEngine actor shutting down");
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
