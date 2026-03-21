use crate::error::DatasetEngineError;
use crate::parquet::tracing::traits::arrow_schema_to_delta;
use crate::parquet::utils::register_cloud_logstore_factories;
use crate::storage::ObjectStore;
use arrow::array::*;
use arrow::datatypes::*;
use arrow_array::RecordBatch;
use chrono::Utc;
use dashmap::DashMap;
use datafusion::prelude::*;
use deltalake::protocol::SaveMode;
use deltalake::{DeltaTable, DeltaTableBuilder, TableProperty};
use scouter_types::dataset::{DatasetNamespace, DatasetRegistration, DatasetStatus};
use std::sync::Arc;
use tokio::sync::RwLock as AsyncRwLock;
use tracing::{debug, info, warn};
use url::Url;

pub(crate) const REGISTRY_TABLE_NAME: &str = "_scouter_dataset_registry";

fn registry_schema() -> Schema {
    Schema::new(vec![
        Field::new("fqn", DataType::Utf8, false),
        Field::new("catalog", DataType::Utf8, false),
        Field::new("schema_name", DataType::Utf8, false),
        Field::new("table_name", DataType::Utf8, false),
        Field::new("fingerprint", DataType::Utf8, false),
        Field::new("arrow_schema_json", DataType::Utf8, false),
        Field::new("json_schema", DataType::Utf8, false),
        Field::new("partition_columns", DataType::Utf8, false),
        Field::new(
            "created_at",
            DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
            false,
        ),
        Field::new(
            "updated_at",
            DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
            false,
        ),
        Field::new("status", DataType::Utf8, false),
    ])
}

fn build_registry_url(object_store: &ObjectStore) -> Result<Url, DatasetEngineError> {
    let mut base = object_store.get_base_url()?;
    let mut path = base.path().to_string();
    if !path.ends_with('/') {
        path.push('/');
    }
    path.push_str("datasets/");
    path.push_str(REGISTRY_TABLE_NAME);
    base.set_path(&path);
    Ok(base)
}

async fn build_or_create_registry(
    object_store: &ObjectStore,
) -> Result<DeltaTable, DatasetEngineError> {
    register_cloud_logstore_factories();
    let table_url = build_registry_url(object_store)?;

    // For local filesystem, create dir if needed
    if table_url.scheme() == "file" {
        if let Ok(path) = table_url.to_file_path() {
            if !path.exists() {
                std::fs::create_dir_all(&path)?;
            }
        }
    }

    // Try to load existing table first
    let store = object_store.as_dyn_object_store();
    match DeltaTableBuilder::from_url(table_url.clone())?
        .with_storage_backend(store.clone(), table_url.clone())
        .load()
        .await
    {
        Ok(table) => {
            info!("Loaded existing dataset registry");
            Ok(table)
        }
        Err(_) => {
            info!("Creating new dataset registry");
            let schema = registry_schema();
            let delta_fields = arrow_schema_to_delta(&schema);

            let table = DeltaTableBuilder::from_url(table_url.clone())?
                .with_storage_backend(store, table_url)
                .build()?;

            let table = table
                .create()
                .with_table_name(REGISTRY_TABLE_NAME)
                .with_columns(delta_fields)
                .with_configuration_property(TableProperty::CheckpointInterval, Some("5"))
                .await?;

            Ok(table)
        }
    }
}

fn build_registration_batch(
    schema: &SchemaRef,
    reg: &DatasetRegistration,
) -> Result<RecordBatch, DatasetEngineError> {
    let now = Utc::now().timestamp_micros();
    let partition_cols_json =
        serde_json::to_string(&reg.partition_columns).map_err(|e| {
            DatasetEngineError::SerializationError(format!(
                "Failed to serialize partition_columns: {}",
                e
            ))
        })?;

    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(StringArray::from(vec![reg.namespace.fqn()])),
            Arc::new(StringArray::from(vec![reg.namespace.catalog.as_str()])),
            Arc::new(StringArray::from(vec![reg.namespace.schema_name.as_str()])),
            Arc::new(StringArray::from(vec![reg.namespace.table.as_str()])),
            Arc::new(StringArray::from(vec![reg.fingerprint.as_str()])),
            Arc::new(StringArray::from(vec![reg.arrow_schema_json.as_str()])),
            Arc::new(StringArray::from(vec![reg.json_schema.as_str()])),
            Arc::new(StringArray::from(vec![partition_cols_json.as_str()])),
            Arc::new(
                TimestampMicrosecondArray::from(vec![now])
                    .with_timezone("UTC"),
            ),
            Arc::new(
                TimestampMicrosecondArray::from(vec![now])
                    .with_timezone("UTC"),
            ),
            Arc::new(StringArray::from(vec![reg.status.to_string().as_str()])),
        ],
    )?;

    Ok(batch)
}

/// Persistent schema registry backed by a Delta Lake table.
///
/// Stores all dataset registrations with a DashMap hot cache for O(1)
/// lookups on the write path (fingerprint validation).
pub struct DatasetRegistry {
    table: Arc<AsyncRwLock<DeltaTable>>,
    ctx: Arc<SessionContext>,
    _object_store: ObjectStore,
    schema: SchemaRef,
    cache: DashMap<String, DatasetRegistration>,
}

impl DatasetRegistry {
    pub async fn new(
        object_store: &ObjectStore,
    ) -> Result<Self, DatasetEngineError> {
        let delta_table = build_or_create_registry(object_store).await?;
        let ctx = object_store.get_session()?;
        let schema = Arc::new(registry_schema());

        // Register object store bindings first
        delta_table.update_datafusion_session(&ctx.state())?;

        match delta_table.table_provider().await {
            Ok(provider) => {
                ctx.register_table(REGISTRY_TABLE_NAME, provider)?;
                info!(
                    "Registry table registered (version: {:?})",
                    delta_table.version()
                );
            }
            Err(e) => {
                info!("Registry table provider unavailable (likely new/empty): {}", e);
            }
        }

        let registry = Self {
            table: Arc::new(AsyncRwLock::new(delta_table)),
            ctx: Arc::new(ctx),
            _object_store: object_store.clone(),
            schema,
            cache: DashMap::new(),
        };

        registry.load_all().await?;

        Ok(registry)
    }

    /// Load all active registrations from the Delta table into the cache.
    pub async fn load_all(&self) -> Result<(), DatasetEngineError> {
        // Refresh the table to get the latest state
        {
            let mut table_guard = self.table.write().await;
            // Try to refresh — picks up commits from other processes
            let _ = table_guard.update_incremental(None).await;
            // Register object store with the DataFusion session so DeltaScan
            // can resolve file URLs during query execution.
            let _ = table_guard.update_datafusion_session(&self.ctx.state());
            let _ = self.ctx.deregister_table(REGISTRY_TABLE_NAME);
            match table_guard.table_provider().await {
                Ok(provider) => {
                    self.ctx.register_table(REGISTRY_TABLE_NAME, provider)?;
                }
                Err(_) => {
                    // Empty or new table — no data to load
                    return Ok(());
                }
            }
        }

        let df = match self.ctx.sql(&format!(
            "SELECT * FROM {}",
            REGISTRY_TABLE_NAME
        )).await {
            Ok(df) => df,
            Err(e) => {
                info!("Registry query failed (likely empty table): {}", e);
                return Ok(());
            }
        };

        let batches = df.collect().await?;

        for batch in &batches {
            // session config has schema_force_view_types=true → Utf8 reads back as Utf8View
            let fqn_col = batch
                .column_by_name("fqn")
                .and_then(|c| c.as_string_view_opt());
            let catalog_col = batch
                .column_by_name("catalog")
                .and_then(|c| c.as_string_view_opt());
            let schema_name_col = batch
                .column_by_name("schema_name")
                .and_then(|c| c.as_string_view_opt());
            let table_name_col = batch
                .column_by_name("table_name")
                .and_then(|c| c.as_string_view_opt());
            let fingerprint_col = batch
                .column_by_name("fingerprint")
                .and_then(|c| c.as_string_view_opt());
            let arrow_schema_col = batch
                .column_by_name("arrow_schema_json")
                .and_then(|c| c.as_string_view_opt());
            let json_schema_col = batch
                .column_by_name("json_schema")
                .and_then(|c| c.as_string_view_opt());
            let partition_col = batch
                .column_by_name("partition_columns")
                .and_then(|c| c.as_string_view_opt());

            let (
                Some(fqn_col),
                Some(catalog_col),
                Some(schema_name_col),
                Some(table_name_col),
                Some(fingerprint_col),
                Some(arrow_schema_col),
                Some(json_schema_col),
                Some(partition_col),
            ) = (
                fqn_col,
                catalog_col,
                schema_name_col,
                table_name_col,
                fingerprint_col,
                arrow_schema_col,
                json_schema_col,
                partition_col,
            )
            else {
                warn!("Registry batch missing expected columns — skipping");
                continue;
            };

            for i in 0..batch.num_rows() {
                let fqn = fqn_col.value(i).to_string();
                let namespace = match DatasetNamespace::new(
                    catalog_col.value(i),
                    schema_name_col.value(i),
                    table_name_col.value(i),
                ) {
                    Ok(ns) => ns,
                    Err(e) => {
                        warn!("Invalid namespace in registry row {}: {}", i, e);
                        continue;
                    }
                };

                let partition_columns: Vec<String> =
                    serde_json::from_str(partition_col.value(i)).unwrap_or_default();

                let reg = DatasetRegistration {
                    namespace,
                    fingerprint: scouter_types::dataset::DatasetFingerprint(
                        fingerprint_col.value(i).to_string(),
                    ),
                    arrow_schema_json: arrow_schema_col.value(i).to_string(),
                    json_schema: json_schema_col.value(i).to_string(),
                    partition_columns,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                    status: DatasetStatus::Active,
                };

                self.cache.insert(fqn, reg);
            }
        }

        info!("Loaded {} registrations from registry", self.cache.len());
        Ok(())
    }

    /// Register a dataset. Idempotent:
    /// - Not found → create → "created"
    /// - Found + fingerprint match → "already_exists"
    /// - Found + fingerprint mismatch → error
    pub async fn register(
        &self,
        registration: &DatasetRegistration,
    ) -> Result<RegistrationResult, DatasetEngineError> {
        let fqn = registration.namespace.fqn();

        // Check cache first
        if let Some(existing) = self.cache.get(&fqn) {
            if existing.fingerprint.as_str() == registration.fingerprint.as_str() {
                return Ok(RegistrationResult::AlreadyExists);
            } else {
                return Err(DatasetEngineError::FingerprintMismatch {
                    table: fqn,
                    expected: existing.fingerprint.as_str().to_string(),
                    actual: registration.fingerprint.as_str().to_string(),
                });
            }
        }

        // Write to Delta table
        let batch = build_registration_batch(&self.schema, registration)?;
        let mut table_guard = self.table.write().await;

        let updated_table = table_guard
            .clone()
            .write(vec![batch])
            .with_save_mode(SaveMode::Append)
            .await?;

        let _ = self.ctx.deregister_table(REGISTRY_TABLE_NAME);
        if let Ok(provider) = updated_table.table_provider().await {
            self.ctx
                .register_table(REGISTRY_TABLE_NAME, provider)?;
        }
        updated_table.update_datafusion_session(&self.ctx.state())?;

        *table_guard = updated_table;

        // Update cache
        self.cache.insert(fqn, registration.clone());

        Ok(RegistrationResult::Created)
    }

    /// O(1) lookup by FQN from cache.
    pub fn get(&self, fqn: &str) -> Option<DatasetRegistration> {
        self.cache.get(fqn).map(|r| r.clone())
    }

    /// Get registration by namespace.
    pub fn get_by_namespace(
        &self,
        namespace: &DatasetNamespace,
    ) -> Option<DatasetRegistration> {
        self.get(&namespace.fqn())
    }

    /// List all active registrations from cache.
    pub fn list_active(&self) -> Vec<DatasetRegistration> {
        self.cache
            .iter()
            .filter(|e| matches!(e.value().status, DatasetStatus::Active))
            .map(|e| e.value().clone())
            .collect()
    }

    /// Refresh from Delta table to pick up registrations from other pods.
    pub async fn refresh(&self) -> Result<(), DatasetEngineError> {
        let mut table_guard = self.table.write().await;
        match table_guard.update_incremental(None).await {
            Ok(_) => {
                let _ = self.ctx.deregister_table(REGISTRY_TABLE_NAME);
                if let Ok(provider) = table_guard.table_provider().await {
                    self.ctx
                        .register_table(REGISTRY_TABLE_NAME, provider)?;
                }
                drop(table_guard);
                self.load_all().await?;
            }
            Err(e) => {
                debug!("Registry refresh skipped: {}", e);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum RegistrationResult {
    Created,
    AlreadyExists,
}
