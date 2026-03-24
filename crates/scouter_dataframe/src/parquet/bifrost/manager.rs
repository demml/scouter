use crate::error::DatasetEngineError;
use crate::parquet::bifrost::buffer::start_buffer;
use crate::parquet::bifrost::catalog::DatasetCatalogProvider;
use crate::parquet::bifrost::engine::{DatasetEngine, TableCommand};
use crate::parquet::bifrost::explain::{
    logical_plan_to_tree, physical_plan_to_tree, sanitize_plan_text, ExplainResult,
};
use crate::parquet::bifrost::query::{QueryExecutionMetadata, QueryResult, QueryTracker};
use crate::parquet::bifrost::registry::{DatasetRegistry, RegistrationResult};
use crate::parquet::bifrost::stats;
use crate::storage::ObjectStore;
use arrow::datatypes::SchemaRef;
use arrow_array::RecordBatch;
use dashmap::DashMap;
use datafusion::physical_plan::displayable;
use datafusion::prelude::SessionContext;
use scouter_settings::ObjectStorageSettings;
use scouter_types::dataset::schema::{
    SCOUTER_BATCH_ID, SCOUTER_CREATED_AT, SCOUTER_PARTITION_DATE,
};
use scouter_types::dataset::{DatasetFingerprint, DatasetNamespace, DatasetRegistration};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Mutex, Notify};
use tokio::time::{interval, Duration};
use tracing::{info, warn};

const DEFAULT_ENGINE_TTL_SECS: u64 = 30 * 60; // 30 minutes
const DEFAULT_MAX_ACTIVE_ENGINES: usize = 50;
const DEFAULT_FLUSH_INTERVAL_SECS: u64 = 60;
const DEFAULT_MAX_BUFFER_ROWS: usize = 10_000;
const DEFAULT_REFRESH_INTERVAL_SECS: u64 = 30;
const REAPER_INTERVAL_SECS: u64 = 5 * 60; // 5 minutes
const DISCOVERY_INTERVAL_SECS: u64 = 60;

pub struct DatasetTableHandle {
    pub buffer_tx: mpsc::Sender<RecordBatch>,
    pub engine_tx: mpsc::Sender<TableCommand>,
    shutdown_tx: mpsc::Sender<()>,
    pub schema: SchemaRef,
    pub fingerprint: DatasetFingerprint,
    pub namespace: DatasetNamespace,
    pub partition_columns: Vec<String>,
    pub last_active_at: Arc<AtomicI64>,
    engine_handle: tokio::task::JoinHandle<()>,
    buffer_handle: tokio::task::JoinHandle<()>,
}

impl DatasetTableHandle {
    fn touch(&self) {
        self.last_active_at
            .store(chrono::Utc::now().timestamp(), Ordering::Relaxed);
    }
}

/// Top-level coordinator for all dataset tables.
///
/// Manages a registry of table metadata, lazy-loads engine actors on demand,
/// and evicts idle engines based on TTL and a hard cap.
pub struct DatasetEngineManager {
    registry: Arc<DatasetRegistry>,
    active_engines: Arc<DashMap<String, DatasetTableHandle>>,
    activating: Arc<Mutex<HashMap<String, Arc<Notify>>>>,
    query_ctx: Arc<SessionContext>,
    catalog_provider: Arc<DatasetCatalogProvider>,
    object_store: ObjectStore,
    query_tracker: QueryTracker,
    engine_ttl_secs: u64,
    max_active_engines: usize,
    flush_interval_secs: u64,
    max_buffer_rows: usize,
    refresh_interval_secs: u64,
}

/// Validate that a SQL string contains exactly one SELECT statement.
/// Rejects DDL, DML, SHOW, and DataFusion extension statements.
fn validate_sql(sql: &str) -> Result<(), DatasetEngineError> {
    use datafusion::sql::parser::{DFParser, Statement as DFStatement};
    use datafusion::sql::sqlparser::ast::Statement as SqlStatement;

    let statements = DFParser::parse_sql(sql)
        .map_err(|e| DatasetEngineError::SqlValidationError(format!("Failed to parse SQL: {e}")))?;

    if statements.len() != 1 {
        return Err(DatasetEngineError::SqlValidationError(
            "Exactly one SQL statement is required".to_string(),
        ));
    }

    match &statements[0] {
        DFStatement::Statement(stmt) => match stmt.as_ref() {
            SqlStatement::Query(_) => Ok(()),
            // Explicitly deny write-capable and DDL variants as defense-in-depth
            SqlStatement::Copy { .. }
            | SqlStatement::CreateTable(_)
            | SqlStatement::Drop { .. }
            | SqlStatement::Insert(_)
            | SqlStatement::Update { .. }
            | SqlStatement::Delete(_) => Err(DatasetEngineError::SqlValidationError(
                "DDL and DML statements are not permitted".to_string(),
            )),
            other => Err(DatasetEngineError::SqlValidationError(format!(
                "Only SELECT queries are allowed, got: {}",
                other
            ))),
        },
        _ => Err(DatasetEngineError::SqlValidationError(
            "Only SELECT queries are allowed".to_string(),
        )),
    }
}

impl DatasetEngineManager {
    pub async fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, DatasetEngineError> {
        let object_store = ObjectStore::new(storage_settings)?;
        let query_ctx = Arc::new(object_store.get_session()?);
        let catalog_provider = Arc::new(DatasetCatalogProvider::new());

        // Register our catalog provider for each known catalog
        // (catalogs are discovered dynamically as tables are registered)

        let registry = Arc::new(DatasetRegistry::new(&object_store).await?);

        let manager = Self {
            registry,
            active_engines: Arc::new(DashMap::new()),
            activating: Arc::new(Mutex::new(HashMap::new())),
            query_ctx,
            catalog_provider,
            object_store,
            query_tracker: QueryTracker::new(),
            engine_ttl_secs: DEFAULT_ENGINE_TTL_SECS,
            max_active_engines: DEFAULT_MAX_ACTIVE_ENGINES,
            flush_interval_secs: DEFAULT_FLUSH_INTERVAL_SECS,
            max_buffer_rows: DEFAULT_MAX_BUFFER_ROWS,
            refresh_interval_secs: DEFAULT_REFRESH_INTERVAL_SECS,
        };

        // Pre-register catalog names from existing registrations so DataFusion
        // can resolve them. No engines are spawned — all lazy-loaded.
        for reg in manager.registry.list_active() {
            manager.ensure_catalog_registered(&reg.namespace.catalog);
        }

        Ok(manager)
    }

    /// Create a manager with custom configuration (primarily for testing).
    pub async fn with_config(
        storage_settings: &ObjectStorageSettings,
        engine_ttl_secs: u64,
        max_active_engines: usize,
        flush_interval_secs: u64,
        max_buffer_rows: usize,
        refresh_interval_secs: u64,
    ) -> Result<Self, DatasetEngineError> {
        let mut manager = Self::new(storage_settings).await?;
        manager.engine_ttl_secs = engine_ttl_secs;
        manager.max_active_engines = max_active_engines;
        manager.flush_interval_secs = flush_interval_secs;
        manager.max_buffer_rows = max_buffer_rows;
        manager.refresh_interval_secs = refresh_interval_secs;
        Ok(manager)
    }

    /// Register a dataset schema. Idempotent.
    /// Does NOT spawn an engine — that's lazy on first write/query.
    pub async fn register_dataset(
        &self,
        registration: &DatasetRegistration,
    ) -> Result<RegistrationResult, DatasetEngineError> {
        let result = self.registry.register(registration).await?;
        self.ensure_catalog_registered(&registration.namespace.catalog);
        Ok(result)
    }

    /// Get or activate an engine for the given namespace.
    /// If the engine is already active, touches `last_active_at` and returns the handle reference.
    /// If not active, lazy-loads the Delta table, spawns the actor pair, and returns it.
    ///
    /// Uses `activating` to prevent two concurrent callers from creating duplicate engine
    /// actors for the same FQN (TOCTOU guard).
    ///
    /// Cancellation safety: a spawned cleanup task holds an `Arc` clone of `activating`
    /// and waits for a oneshot signal. If this future is dropped mid-await, the oneshot
    /// sender drops too, the cleanup task unblocks via `Err(RecvError)`, and it removes
    /// the stale entry + fires `notify_waiters()` so waiters are not permanently hung.
    async fn activate_engine(
        &self,
        namespace: &DatasetNamespace,
    ) -> Result<(), DatasetEngineError> {
        let fqn = namespace.fqn();

        // Fast path: already active
        if let Some(handle) = self.active_engines.get(&fqn) {
            handle.touch();
            return Ok(());
        }

        // Serialize concurrent activations for the same FQN
        {
            let mut pending = self.activating.lock().await;

            // Re-check after acquiring lock — another task may have completed activation
            if let Some(handle) = self.active_engines.get(&fqn) {
                handle.touch();
                return Ok(());
            }

            if let Some(notify) = pending.get(&fqn) {
                // Another task is already activating this FQN — wait for it.
                // Pin and enable the Notified future before releasing the lock so the
                // waiter is registered before the activating task can call notify_waiters().
                // Without enable(), a notify_waiters() fired between drop(pending) and the
                // first poll of notified.await would be lost, hanging this task forever.
                let notify = Arc::clone(notify);
                let notified = notify.notified();
                tokio::pin!(notified);
                notified.as_mut().enable();
                drop(pending);

                // Timeout bounds worst-case wait if the cleanup task is somehow delayed.
                match tokio::time::timeout(Duration::from_secs(30), notified).await {
                    Ok(_) => {}
                    Err(_) => {
                        return Err(DatasetEngineError::RegistryError(format!(
                            "Engine activation timed out for {fqn}"
                        )));
                    }
                }

                return if self.active_engines.contains_key(&fqn) {
                    Ok(())
                } else {
                    Err(DatasetEngineError::RegistryError(format!(
                        "Activation failed for {fqn}"
                    )))
                };
            }

            pending.insert(fqn.clone(), Arc::new(Notify::new()));
        } // activating lock released — safe to .await

        // Spawn a cleanup task that runs whether we complete normally or get cancelled.
        // The oneshot sender is dropped in both cases, unblocking the cleanup task.
        let (done_tx, done_rx) = tokio::sync::oneshot::channel::<()>();
        let activating = Arc::clone(&self.activating);
        let fqn_for_cleanup = fqn.clone();
        tokio::spawn(async move {
            // Waits for done_tx.send(()) on the happy path, or Err if sender is dropped
            // (future cancelled). Either way, clean up the pending entry.
            let _ = done_rx.await;
            let mut pending = activating.lock().await;
            if let Some(notify) = pending.remove(&fqn_for_cleanup) {
                notify.notify_waiters();
            }
        });

        let result = self.do_activate_engine_inner(namespace, &fqn).await;
        let _ = done_tx.send(()); // signal cleanup; if already dropped, cleanup already ran
        result
    }

    /// Inner activation logic, called only when we hold the pending-set reservation.
    async fn do_activate_engine_inner(
        &self,
        namespace: &DatasetNamespace,
        fqn: &str,
    ) -> Result<(), DatasetEngineError> {
        // Look up registration
        let reg = self
            .registry
            .get(fqn)
            .ok_or_else(|| DatasetEngineError::TableNotFound(fqn.to_string()))?;

        // Check cap — evict LRU if needed
        if self.active_engines.len() >= self.max_active_engines {
            self.evict_lru().await;
        }

        // Parse the Arrow schema from the registration
        let arrow_schema: arrow::datatypes::Schema = serde_json::from_str(&reg.arrow_schema_json)
            .map_err(|e| {
            DatasetEngineError::SerializationError(format!(
                "Failed to deserialize Arrow schema for {}: {}",
                fqn, e
            ))
        })?;
        let schema = Arc::new(arrow_schema);

        // Build full partition columns list
        let mut partition_columns = vec![SCOUTER_PARTITION_DATE.to_string()];
        for col in &reg.partition_columns {
            if !partition_columns.contains(col) {
                partition_columns.push(col.clone());
            }
        }

        // Create the engine
        let engine = DatasetEngine::new(
            &self.object_store,
            schema.clone(),
            namespace.clone(),
            partition_columns.clone(),
            Arc::clone(&self.catalog_provider),
        )
        .await?;

        // Start the engine actor
        let (engine_tx, engine_handle) = engine.start_actor(self.refresh_interval_secs);

        // Start the buffer actor
        let (buffer_tx, batch_rx) = mpsc::channel::<RecordBatch>(100);
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>(1);
        let buffer_handle = start_buffer(
            engine_tx.clone(),
            batch_rx,
            shutdown_rx,
            self.flush_interval_secs,
            self.max_buffer_rows,
            fqn.to_string(),
        );

        self.ensure_catalog_registered(&namespace.catalog);

        let handle = DatasetTableHandle {
            buffer_tx,
            engine_tx,
            shutdown_tx,
            schema,
            fingerprint: reg.fingerprint.clone(),
            namespace: namespace.clone(),
            partition_columns,
            last_active_at: Arc::new(AtomicI64::new(chrono::Utc::now().timestamp())),
            engine_handle,
            buffer_handle,
        };

        self.active_engines.insert(fqn.to_string(), handle);
        info!("Activated engine for [{}]", fqn);

        Ok(())
    }

    /// Insert a RecordBatch into a dataset table.
    /// Activates the engine on demand if not already active.
    pub async fn insert_batch(
        &self,
        namespace: &DatasetNamespace,
        fingerprint: &DatasetFingerprint,
        batch: RecordBatch,
    ) -> Result<(), DatasetEngineError> {
        let fqn = namespace.fqn();

        // Activate if needed
        self.activate_engine(namespace).await?;

        let handle = self
            .active_engines
            .get(&fqn)
            .ok_or_else(|| DatasetEngineError::TableNotFound(fqn.clone()))?;

        // Validate fingerprint
        if handle.fingerprint.as_str() != fingerprint.as_str() {
            warn!(
                table = %fqn,
                "Fingerprint mismatch: expected={}, actual={}",
                handle.fingerprint.as_str(),
                fingerprint.as_str()
            );
            return Err(DatasetEngineError::FingerprintMismatch {
                table: fqn,
                expected: handle.fingerprint.as_str().to_string(),
                actual: fingerprint.as_str().to_string(),
            });
        }

        handle.touch();

        // Send to buffer
        handle
            .buffer_tx
            .send(batch)
            .await
            .map_err(|_| DatasetEngineError::ChannelClosed)?;

        Ok(())
    }

    /// Execute a SQL query against the shared query context.
    ///
    /// Only SELECT statements are allowed. All other statement types (DDL, DML,
    /// SHOW, etc.) are rejected at parse time.
    pub async fn query(&self, sql: &str) -> Result<Vec<RecordBatch>, DatasetEngineError> {
        validate_sql(sql)?;
        let df = self.query_ctx.sql(sql).await?;
        let batches = df.collect().await?;
        Ok(batches)
    }

    /// List all registered datasets (from registry cache, not just active engines).
    pub fn list_datasets(&self) -> Vec<DatasetRegistration> {
        self.registry.list_active()
    }

    /// Get registration info for a specific dataset.
    pub fn get_dataset_info(&self, namespace: &DatasetNamespace) -> Option<DatasetRegistration> {
        self.registry.get_by_namespace(namespace)
    }

    // ── Catalog Browser APIs ────────────────────────────────────────────

    /// List all distinct catalogs with schema and table counts.
    pub fn list_catalogs(&self) -> Vec<CatalogSummary> {
        let datasets = self.registry.list_active();
        let mut catalog_map: HashMap<String, (HashSet<String>, u32)> = HashMap::new();

        for d in &datasets {
            let entry = catalog_map
                .entry(d.namespace.catalog.clone())
                .or_insert_with(|| (HashSet::new(), 0));
            entry.0.insert(d.namespace.schema_name.clone());
            entry.1 += 1;
        }

        catalog_map
            .into_iter()
            .map(|(catalog, (schemas, table_count))| CatalogSummary {
                catalog,
                schema_count: schemas.len() as u32,
                table_count,
            })
            .collect()
    }

    /// List schemas within a catalog with table counts.
    pub fn list_schemas(&self, catalog: &str) -> Vec<SchemaSummary> {
        let datasets = self.registry.list_active();
        let mut schema_map: HashMap<String, u32> = HashMap::new();

        for d in datasets.iter().filter(|d| d.namespace.catalog == catalog) {
            *schema_map
                .entry(d.namespace.schema_name.clone())
                .or_insert(0) += 1;
        }

        schema_map
            .into_iter()
            .map(|(schema_name, table_count)| SchemaSummary {
                catalog: catalog.to_string(),
                schema_name,
                table_count,
            })
            .collect()
    }

    /// List tables within a catalog.schema with summary info.
    pub fn list_tables(&self, catalog: &str, schema_name: &str) -> Vec<TableSummaryInfo> {
        self.registry
            .list_active()
            .into_iter()
            .filter(|d| d.namespace.catalog == catalog && d.namespace.schema_name == schema_name)
            .map(|d| TableSummaryInfo {
                catalog: d.namespace.catalog,
                schema_name: d.namespace.schema_name,
                table: d.namespace.table,
                status: d.status.to_string(),
                created_at: d.created_at.to_rfc3339(),
                updated_at: d.updated_at.to_rfc3339(),
            })
            .collect()
    }

    /// Get detailed info for a table: columns, partition info, and Delta stats.
    pub async fn get_table_detail(
        &self,
        namespace: &DatasetNamespace,
    ) -> Result<TableDetail, DatasetEngineError> {
        let reg = self
            .registry
            .get_by_namespace(namespace)
            .ok_or_else(|| DatasetEngineError::TableNotFound(namespace.fqn()))?;

        // Parse Arrow schema from registration
        let arrow_schema: arrow::datatypes::Schema = serde_json::from_str(&reg.arrow_schema_json)
            .map_err(|e| {
            DatasetEngineError::SerializationError(format!(
                "Failed to deserialize Arrow schema: {e}"
            ))
        })?;

        let partition_set: HashSet<&str> =
            reg.partition_columns.iter().map(|s| s.as_str()).collect();
        let system_cols: HashSet<&str> =
            [SCOUTER_CREATED_AT, SCOUTER_PARTITION_DATE, SCOUTER_BATCH_ID]
                .into_iter()
                .collect();

        let columns: Vec<ColumnDetail> = arrow_schema
            .fields()
            .iter()
            .map(|f| ColumnDetail {
                name: f.name().clone(),
                arrow_type: format!("{}", f.data_type()),
                nullable: f.is_nullable(),
                is_partition: partition_set.contains(f.name().as_str()),
                is_system: system_cols.contains(f.name().as_str()),
            })
            .collect();

        // Load stats from Delta log (transient load for inactive tables)
        let table_stats = stats::load_table_stats(&self.object_store, namespace).await?;

        Ok(TableDetail {
            registration: reg,
            columns,
            stats: table_stats,
        })
    }

    /// Preview a table's data (SELECT * LIMIT max_rows).
    pub async fn preview_table(
        &self,
        namespace: &DatasetNamespace,
        max_rows: usize,
    ) -> Result<Vec<RecordBatch>, DatasetEngineError> {
        let max_rows = max_rows.min(1000);
        let sql = format!(
            "SELECT * FROM {} LIMIT {}",
            namespace.quoted_fqn(),
            max_rows
        );
        self.activate_engine(namespace).await?;
        let df = self.query_ctx.sql(&sql).await?;
        let batches = df.collect().await?;
        Ok(batches)
    }

    // ── Enhanced Query Execution ────────────────────────────────────────

    /// Execute a SQL query with row limits, cancellation support, and metadata.
    pub async fn execute_query(
        &self,
        sql: &str,
        query_id: &str,
        max_rows: usize,
    ) -> Result<QueryResult, DatasetEngineError> {
        validate_sql(sql)?;
        let max_rows = max_rows.clamp(1, 100_000);

        let cancel_token = self.query_tracker.register(query_id).await?;
        let start = Instant::now();

        let exec_result: Result<_, DatasetEngineError> = async {
            let df = self.query_ctx.sql(sql).await?;
            // Request max_rows + 1 to detect truncation
            let limited_df = df.limit(0, Some(max_rows + 1))?;
            tokio::select! {
                result = limited_df.collect() => result.map_err(DatasetEngineError::from),
                _ = cancel_token.cancelled() => {
                    Err(DatasetEngineError::QueryCancelled(query_id.to_string()))
                }
            }
        }
        .await;

        self.query_tracker.remove(query_id).await;
        let batches = exec_result?;

        let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
        let truncated = total_rows > max_rows;

        // If truncated, we need to trim the last batch
        let final_batches = if truncated {
            let mut remaining = max_rows;
            let mut result = Vec::new();
            for batch in batches {
                if remaining == 0 {
                    break;
                }
                if batch.num_rows() <= remaining {
                    remaining -= batch.num_rows();
                    result.push(batch);
                } else {
                    result.push(batch.slice(0, remaining));
                    remaining = 0;
                }
            }
            result
        } else {
            batches
        };

        let rows_returned: usize = final_batches.iter().map(|b| b.num_rows()).sum();

        Ok(QueryResult {
            batches: final_batches,
            metadata: QueryExecutionMetadata {
                query_id: query_id.to_string(),
                rows_returned: rows_returned as u64,
                truncated,
                execution_time_ms: start.elapsed().as_millis() as u64,
                bytes_scanned: None,
            },
        })
    }

    /// Cancel a running query by ID.
    pub async fn cancel_query(&self, query_id: &str) -> bool {
        self.query_tracker.cancel(query_id).await
    }

    // ── Query Plan ──────────────────────────────────────────────────────

    /// Generate a structured query plan, optionally with ANALYZE execution.
    pub async fn explain_query(
        &self,
        sql: &str,
        analyze: bool,
        max_rows: usize,
    ) -> Result<ExplainResult, DatasetEngineError> {
        validate_sql(sql)?;
        let df = self.query_ctx.sql(sql).await?;

        // Logical plan (optimized)
        let logical_plan = df.logical_plan().clone();
        let logical_tree = logical_plan_to_tree(&logical_plan);
        let logical_text = sanitize_plan_text(&format!("{}", logical_plan.display_indent()));

        // Physical plan
        let physical_plan = df.create_physical_plan().await?;
        let physical_tree = physical_plan_to_tree(physical_plan.as_ref());
        let physical_text =
            sanitize_plan_text(&displayable(physical_plan.as_ref()).indent(true).to_string());

        let execution_metadata = if analyze {
            let max_rows = max_rows.clamp(1, 100_000);
            let analyze_df = self.query_ctx.sql(sql).await?;
            let limited = analyze_df.limit(0, Some(max_rows + 1))?;
            let start = Instant::now();
            let batches = limited.collect().await?;
            let rows: usize = batches.iter().map(|b| b.num_rows()).sum();

            Some(QueryExecutionMetadata {
                query_id: String::new(),
                rows_returned: rows.min(max_rows) as u64,
                truncated: rows > max_rows,
                execution_time_ms: start.elapsed().as_millis() as u64,
                bytes_scanned: None,
            })
        } else {
            None
        };

        Ok(ExplainResult {
            logical_plan: logical_tree,
            physical_plan: physical_tree,
            logical_plan_text: logical_text,
            physical_plan_text: physical_text,
            execution_metadata,
        })
    }

    /// Evict the least-recently-used engine.
    async fn evict_lru(&self) {
        let lru_fqn = self
            .active_engines
            .iter()
            .min_by_key(|e| e.value().last_active_at.load(Ordering::Relaxed))
            .map(|e| e.key().clone());

        if let Some(fqn) = lru_fqn {
            self.evict_engine(&fqn).await;
        }
    }

    /// Evict a specific engine by FQN.
    async fn evict_engine(&self, fqn: &str) {
        if let Some((_, handle)) = self.active_engines.remove(fqn) {
            info!("Evicting engine [{}]", fqn);

            // 1. Signal buffer to flush remaining batches and exit
            let _ = handle.shutdown_tx.send(()).await;

            // 2. Wait for buffer to complete its final flush before shutting down the engine.
            //    This ensures all buffered Write commands are in the engine channel.
            let _ = handle.buffer_handle.await;

            // 3. Now shut down the engine — all buffered Writes are queued (mpsc FIFO)
            let _ = handle.engine_tx.send(TableCommand::Shutdown).await;
            let _ = handle.engine_handle.await;

            // 4. Remove from catalog
            self.catalog_provider.remove_table(&handle.namespace);
        }
    }

    /// Shutdown all active engines gracefully.
    pub async fn shutdown(&self) {
        info!(
            "Shutting down DatasetEngineManager ({} active engines)",
            self.active_engines.len()
        );

        let fqns: Vec<String> = self
            .active_engines
            .iter()
            .map(|e| e.key().clone())
            .collect();

        for fqn in fqns {
            self.evict_engine(&fqn).await;
        }
    }

    /// Start the reaper loop that evicts idle engines.
    ///
    /// Returns a future suitable for `TaskManager::spawn()`. The loop exits
    /// when the shutdown receiver fires.
    pub fn start_reaper_loop(
        self: &Arc<Self>,
        mut shutdown_rx: tokio::sync::watch::Receiver<()>,
    ) -> impl std::future::Future<Output = ()> + Send + 'static {
        let manager = Arc::clone(self);
        async move {
            let mut ticker = interval(Duration::from_secs(REAPER_INTERVAL_SECS));
            ticker.tick().await; // skip immediate

            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        let now = chrono::Utc::now().timestamp();
                        let ttl = manager.engine_ttl_secs as i64;

                        let to_evict: Vec<String> = manager
                            .active_engines
                            .iter()
                            .filter(|e| now - e.value().last_active_at.load(Ordering::Relaxed) > ttl)
                            .map(|e| e.key().clone())
                            .collect();

                        for fqn in to_evict {
                            manager.evict_engine(&fqn).await;
                        }
                    }
                    _ = shutdown_rx.changed() => {
                        info!("Reaper loop shutting down");
                        break;
                    }
                }
            }
        }
    }

    /// Start the discovery loop that refreshes the registry from other pods.
    ///
    /// Returns a future suitable for `TaskManager::spawn()`. The loop exits
    /// when the shutdown receiver fires.
    pub fn start_discovery_loop(
        self: &Arc<Self>,
        mut shutdown_rx: tokio::sync::watch::Receiver<()>,
    ) -> impl std::future::Future<Output = ()> + Send + 'static {
        let manager = Arc::clone(self);
        async move {
            let mut ticker = interval(Duration::from_secs(DISCOVERY_INTERVAL_SECS));
            ticker.tick().await; // skip immediate

            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        if let Err(e) = manager.registry.refresh().await {
                            warn!("Registry discovery refresh failed: {}", e);
                        }

                        // Register any new catalogs discovered
                        for reg in manager.registry.list_active() {
                            manager.ensure_catalog_registered(&reg.namespace.catalog);
                        }
                    }
                    _ = shutdown_rx.changed() => {
                        info!("Discovery loop shutting down");
                        break;
                    }
                }
            }
        }
    }

    /// Access the shared query context (for Phase 3 gRPC/HTTP integration).
    pub fn query_ctx(&self) -> &Arc<SessionContext> {
        &self.query_ctx
    }

    /// Access the registry (for Phase 3 gRPC/HTTP integration).
    pub fn registry(&self) -> &Arc<DatasetRegistry> {
        &self.registry
    }

    /// Number of currently active engines.
    pub fn active_engine_count(&self) -> usize {
        self.active_engines.len()
    }

    /// Register a catalog name with DataFusion (idempotent).
    fn ensure_catalog_registered(&self, catalog: &str) {
        self.query_ctx.register_catalog(
            catalog,
            Arc::clone(&self.catalog_provider) as Arc<dyn datafusion::catalog::CatalogProvider>,
        );
    }
}

// ── Catalog browser types ──────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct CatalogSummary {
    pub catalog: String,
    pub schema_count: u32,
    pub table_count: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SchemaSummary {
    pub catalog: String,
    pub schema_name: String,
    pub table_count: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TableSummaryInfo {
    pub catalog: String,
    pub schema_name: String,
    pub table: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ColumnDetail {
    pub name: String,
    pub arrow_type: String,
    pub nullable: bool,
    pub is_partition: bool,
    pub is_system: bool,
}

pub struct TableDetail {
    pub registration: DatasetRegistration,
    pub columns: Vec<ColumnDetail>,
    pub stats: stats::TableStats,
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::array::AsArray;
    use arrow::datatypes::{DataType, Field, Int64Type, Schema, TimeUnit};
    use scouter_types::dataset::{DatasetFingerprint, DatasetRegistration};
    use tempfile::TempDir;

    fn test_storage_settings(dir: &TempDir) -> ObjectStorageSettings {
        ObjectStorageSettings {
            storage_uri: dir.path().to_str().unwrap().to_string(),
            storage_type: scouter_types::StorageType::Local,
            region: "us-east-1".to_string(),
            trace_compaction_interval_hours: 24,
            trace_flush_interval_secs: 5,
            trace_refresh_interval_secs: 10,
        }
    }

    fn test_schema() -> Schema {
        Schema::new(vec![
            Field::new("user_id", DataType::Utf8, false),
            Field::new("score", DataType::Float64, false),
            Field::new("model_name", DataType::Utf8, true),
            // System columns
            Field::new(
                "scouter_created_at",
                DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
                false,
            ),
            Field::new("scouter_partition_date", DataType::Date32, false),
            Field::new("scouter_batch_id", DataType::Utf8, false),
        ])
    }

    fn test_registration(schema: &Schema) -> DatasetRegistration {
        let arrow_schema_json = serde_json::to_string(schema).unwrap();
        let fingerprint = DatasetFingerprint::from_schema_json(&arrow_schema_json);
        let namespace =
            DatasetNamespace::new("test_catalog", "test_schema", "predictions").unwrap();

        DatasetRegistration::new(
            namespace,
            fingerprint,
            arrow_schema_json,
            "{}".to_string(),
            vec![],
        )
    }

    fn make_test_batch(schema: &Schema) -> RecordBatch {
        use arrow::array::*;
        use chrono::{Datelike, Utc};

        let now = Utc::now();
        let epoch_days = now.date_naive().num_days_from_ce() - 719_163;

        RecordBatch::try_new(
            Arc::new(schema.clone()),
            vec![
                Arc::new(StringArray::from(vec!["user_1", "user_2", "user_3"])),
                Arc::new(Float64Array::from(vec![0.95, 0.87, 0.92])),
                Arc::new(StringArray::from(vec![
                    Some("model_a"),
                    None,
                    Some("model_b"),
                ])),
                Arc::new(
                    TimestampMicrosecondArray::from(vec![
                        now.timestamp_micros(),
                        now.timestamp_micros(),
                        now.timestamp_micros(),
                    ])
                    .with_timezone("UTC"),
                ),
                Arc::new(Date32Array::from(vec![epoch_days, epoch_days, epoch_days])),
                Arc::new(StringArray::from(vec![
                    "batch-001",
                    "batch-001",
                    "batch-001",
                ])),
            ],
        )
        .unwrap()
    }

    #[tokio::test]
    async fn test_register_and_insert() {
        let dir = TempDir::new().unwrap();
        let settings = test_storage_settings(&dir);

        let manager = DatasetEngineManager::with_config(&settings, 1800, 10, 1, 100, 30)
            .await
            .unwrap();

        let schema = test_schema();
        let reg = test_registration(&schema);

        // Register
        let result = manager.register_dataset(&reg).await.unwrap();
        assert_eq!(result, RegistrationResult::Created);

        // Idempotent re-register
        let result2 = manager.register_dataset(&reg).await.unwrap();
        assert_eq!(result2, RegistrationResult::AlreadyExists);

        // No engines spawned yet (lazy)
        assert_eq!(manager.active_engine_count(), 0);

        // Insert a batch — triggers lazy activation
        let batch = make_test_batch(&schema);
        manager
            .insert_batch(&reg.namespace, &reg.fingerprint, batch)
            .await
            .unwrap();

        // Engine should now be active
        assert_eq!(manager.active_engine_count(), 1);

        // Wait for buffer to flush (flush interval = 1s in test config)
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Shutdown cleanly
        manager.shutdown().await;
        assert_eq!(manager.active_engine_count(), 0);
    }

    #[tokio::test]
    async fn test_fingerprint_mismatch() {
        let dir = TempDir::new().unwrap();
        let settings = test_storage_settings(&dir);

        let manager = DatasetEngineManager::with_config(&settings, 1800, 10, 1, 100, 30)
            .await
            .unwrap();

        let schema = test_schema();
        let reg = test_registration(&schema);
        manager.register_dataset(&reg).await.unwrap();

        // Try inserting with wrong fingerprint
        let wrong_fp = DatasetFingerprint::from_schema_json("wrong");
        let batch = make_test_batch(&schema);

        let result = manager.insert_batch(&reg.namespace, &wrong_fp, batch).await;

        assert!(result.is_err());
        if let Err(DatasetEngineError::FingerprintMismatch { .. }) = result {
            // expected
        } else {
            panic!("Expected FingerprintMismatch error");
        }

        manager.shutdown().await;
    }

    #[tokio::test]
    async fn test_table_not_found() {
        let dir = TempDir::new().unwrap();
        let settings = test_storage_settings(&dir);

        let manager = DatasetEngineManager::with_config(&settings, 1800, 10, 1, 100, 30)
            .await
            .unwrap();

        let ns = DatasetNamespace::new("no", "such", "table").unwrap();
        let fp = DatasetFingerprint::from_schema_json("x");
        let schema = test_schema();
        let batch = make_test_batch(&schema);

        let result = manager.insert_batch(&ns, &fp, batch).await;
        assert!(matches!(result, Err(DatasetEngineError::TableNotFound(_))));

        manager.shutdown().await;
    }

    #[tokio::test]
    async fn test_list_datasets() {
        let dir = TempDir::new().unwrap();
        let settings = test_storage_settings(&dir);

        let manager = DatasetEngineManager::with_config(&settings, 1800, 10, 1, 100, 30)
            .await
            .unwrap();

        assert!(manager.list_datasets().is_empty());

        let schema = test_schema();
        let reg = test_registration(&schema);
        manager.register_dataset(&reg).await.unwrap();

        let datasets = manager.list_datasets();
        assert_eq!(datasets.len(), 1);
        assert_eq!(
            datasets[0].namespace.fqn(),
            "test_catalog.test_schema.predictions"
        );

        manager.shutdown().await;
    }

    #[tokio::test]
    async fn test_multiple_tables_isolation() {
        let dir = TempDir::new().unwrap();
        let settings = test_storage_settings(&dir);

        let manager = DatasetEngineManager::with_config(&settings, 1800, 10, 1, 100, 30)
            .await
            .unwrap();

        let schema = test_schema();

        // Register two different tables
        let ns1 = DatasetNamespace::new("cat", "sch", "table_a").unwrap();
        let ns2 = DatasetNamespace::new("cat", "sch", "table_b").unwrap();
        let arrow_json = serde_json::to_string(&schema).unwrap();
        let fp = DatasetFingerprint::from_schema_json(&arrow_json);

        let reg1 = DatasetRegistration::new(
            ns1.clone(),
            fp.clone(),
            arrow_json.clone(),
            "{}".into(),
            vec![],
        );
        let reg2 = DatasetRegistration::new(
            ns2.clone(),
            fp.clone(),
            arrow_json.clone(),
            "{}".into(),
            vec![],
        );

        manager.register_dataset(&reg1).await.unwrap();
        manager.register_dataset(&reg2).await.unwrap();

        // Insert into both
        let batch1 = make_test_batch(&schema);
        let batch2 = make_test_batch(&schema);
        manager.insert_batch(&ns1, &fp, batch1).await.unwrap();
        manager.insert_batch(&ns2, &fp, batch2).await.unwrap();

        assert_eq!(manager.active_engine_count(), 2);

        manager.shutdown().await;
    }

    #[tokio::test]
    async fn test_max_active_engines_cap() {
        let dir = TempDir::new().unwrap();
        let settings = test_storage_settings(&dir);

        // Cap at 2 active engines
        let manager = DatasetEngineManager::with_config(&settings, 1800, 2, 1, 100, 30)
            .await
            .unwrap();

        let schema = test_schema();
        let arrow_json = serde_json::to_string(&schema).unwrap();
        let fp = DatasetFingerprint::from_schema_json(&arrow_json);

        // Register 3 tables
        for i in 0..3 {
            let ns = DatasetNamespace::new("cat", "sch", format!("tbl_{i}")).unwrap();
            let reg =
                DatasetRegistration::new(ns, fp.clone(), arrow_json.clone(), "{}".into(), vec![]);
            manager.register_dataset(&reg).await.unwrap();
        }

        // Activate first two
        let ns0 = DatasetNamespace::new("cat", "sch", "tbl_0").unwrap();
        let ns1 = DatasetNamespace::new("cat", "sch", "tbl_1").unwrap();
        let ns2 = DatasetNamespace::new("cat", "sch", "tbl_2").unwrap();

        manager
            .insert_batch(&ns0, &fp, make_test_batch(&schema))
            .await
            .unwrap();
        manager
            .insert_batch(&ns1, &fp, make_test_batch(&schema))
            .await
            .unwrap();

        assert_eq!(manager.active_engine_count(), 2);

        // Third should evict the LRU
        manager
            .insert_batch(&ns2, &fp, make_test_batch(&schema))
            .await
            .unwrap();

        // Still at cap
        assert_eq!(manager.active_engine_count(), 2);

        manager.shutdown().await;
    }

    #[tokio::test]
    async fn test_write_and_query() {
        let dir = TempDir::new().unwrap();
        let settings = test_storage_settings(&dir);

        let manager = DatasetEngineManager::with_config(
            &settings, 1800, 10, 1,   // 1s flush interval
            100, // small buffer for testing
            30,
        )
        .await
        .unwrap();

        let schema = test_schema();
        let reg = test_registration(&schema);
        manager.register_dataset(&reg).await.unwrap();

        // Insert data
        let batch = make_test_batch(&schema);
        manager
            .insert_batch(&reg.namespace, &reg.fingerprint, batch)
            .await
            .unwrap();

        // Wait for buffer flush + write
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Query via three-level name
        let sql = "SELECT COUNT(*) as cnt FROM test_catalog.test_schema.predictions";
        let results = manager.query(sql).await.unwrap();

        assert!(!results.is_empty());
        let count_col = results[0]
            .column_by_name("cnt")
            .unwrap()
            .as_primitive_opt::<Int64Type>()
            .unwrap();
        assert_eq!(count_col.value(0), 3);

        manager.shutdown().await;
    }

    #[tokio::test]
    async fn test_registry_persistence() {
        let dir = TempDir::new().unwrap();
        let settings = test_storage_settings(&dir);

        // Register a dataset
        {
            let manager = DatasetEngineManager::with_config(&settings, 1800, 10, 1, 100, 30)
                .await
                .unwrap();

            let schema = test_schema();
            let reg = test_registration(&schema);
            manager.register_dataset(&reg).await.unwrap();
            manager.shutdown().await;
        }

        // Create a new manager from same storage — should find the registration
        {
            let manager = DatasetEngineManager::with_config(&settings, 1800, 10, 1, 100, 30)
                .await
                .unwrap();

            let datasets = manager.list_datasets();
            assert_eq!(datasets.len(), 1);
            assert_eq!(
                datasets[0].namespace.fqn(),
                "test_catalog.test_schema.predictions"
            );

            manager.shutdown().await;
        }
    }
}
