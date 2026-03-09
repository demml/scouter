use crate::error::TraceEngineError;
use crate::parquet::tracing::traits::arrow_schema_to_delta;
use crate::storage::ObjectStore;
use arrow::array::*;
use arrow::datatypes::*;
use arrow_array::RecordBatch;
use chrono::{DateTime, Duration, Utc};
use datafusion::logical_expr::{col, lit};
use datafusion::prelude::SessionContext;
use deltalake::logstore::logstore_factories;
use deltalake::{DeltaTable, DeltaTableBuilder, TableProperty};
use scouter_settings::ObjectStorageSettings;
use std::sync::Arc;
use tokio::sync::RwLock as AsyncRwLock;
use tracing::{debug, info, warn};
use url::Url;

const CONTROL_TABLE_NAME: &str = "_scouter_control";

/// Stale lock threshold: if a task has been "processing" for longer than this,
/// it is considered abandoned and can be reclaimed by another pod.
const STALE_LOCK_MINUTES: i64 = 30;

/// Status values for task records in the control table.
mod status {
    pub const IDLE: &str = "idle";
    pub const PROCESSING: &str = "processing";
}

/// A task record in the control table.
#[derive(Debug, Clone)]
pub struct TaskRecord {
    pub task_name: String,
    pub status: String,
    pub pod_id: String,
    pub claimed_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub next_run_at: DateTime<Utc>,
}

/// Arrow schema for the control table.
///
/// Simple, flat schema — no dictionary encoding or bloom filters needed.
/// This table will only ever have a handful of rows (one per task type).
fn control_schema() -> Schema {
    Schema::new(vec![
        Field::new("task_name", DataType::Utf8, false),
        Field::new("status", DataType::Utf8, false),
        Field::new("pod_id", DataType::Utf8, false),
        Field::new(
            "claimed_at",
            DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
            false,
        ),
        Field::new(
            "completed_at",
            DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
            true,
        ),
        Field::new(
            "next_run_at",
            DataType::Timestamp(TimeUnit::Microsecond, Some("UTC".into())),
            false,
        ),
    ])
}

fn build_task_batch(
    schema: &SchemaRef,
    record: &TaskRecord,
) -> Result<RecordBatch, TraceEngineError> {
    let task_name = StringArray::from(vec![record.task_name.as_str()]);
    let status = StringArray::from(vec![record.status.as_str()]);
    let pod_id = StringArray::from(vec![record.pod_id.as_str()]);
    let claimed_at = TimestampMicrosecondArray::from(vec![record.claimed_at.timestamp_micros()])
        .with_timezone("UTC");
    let completed_at = if let Some(ts) = record.completed_at {
        TimestampMicrosecondArray::from(vec![Some(ts.timestamp_micros())]).with_timezone("UTC")
    } else {
        TimestampMicrosecondArray::from(vec![None::<i64>]).with_timezone("UTC")
    };
    let next_run_at = TimestampMicrosecondArray::from(vec![record.next_run_at.timestamp_micros()])
        .with_timezone("UTC");

    RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(task_name),
            Arc::new(status),
            Arc::new(pod_id),
            Arc::new(claimed_at),
            Arc::new(completed_at),
            Arc::new(next_run_at),
        ],
    )
    .map_err(Into::into)
}

/// Get a stable pod identifier for distributed locking.
///
/// Resolution order: `HOSTNAME` (K8s default) → `POD_NAME` (custom override) →
/// `local-{pid}` for local dev.
pub fn get_pod_id() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("POD_NAME"))
        .unwrap_or_else(|_| format!("local-{}", std::process::id()))
}

/// Distributed coordination engine backed by a Delta Lake table.
///
/// Uses Delta Lake's optimistic concurrency control (OCC) to implement
/// distributed task locking across multiple K8s pods. The transaction log
/// serializes concurrent claims — if two pods race to claim the same task,
/// one commit succeeds and the other gets a `TransactionError`.
///
/// The control table lives at `{storage_uri}/_scouter_control/` alongside
/// the trace data tables.
pub struct ControlTableEngine {
    schema: SchemaRef,
    #[allow(dead_code)] // Used for future vacuum/maintenance operations
    object_store: ObjectStore,
    table: Arc<AsyncRwLock<DeltaTable>>,
    ctx: Arc<SessionContext>,
    pod_id: String,
}

impl ControlTableEngine {
    /// Create or load the control table.
    ///
    /// The `pod_id` identifies this instance for distributed locking. In K8s,
    /// pass the pod hostname (`std::env::var("HOSTNAME")`).
    pub async fn new(
        storage_settings: &ObjectStorageSettings,
        pod_id: String,
    ) -> Result<Self, TraceEngineError> {
        let object_store = ObjectStore::new(storage_settings)?;
        let schema = Arc::new(control_schema());
        let table = build_or_create_control_table(&object_store, schema.clone()).await?;
        let ctx = object_store.get_session()?;

        if let Ok(provider) = table.table_provider().await {
            ctx.register_table(CONTROL_TABLE_NAME, provider)?;
        } else {
            info!("Empty control table at init — deferring registration until first write");
        }

        Ok(Self {
            schema,
            object_store,
            table: Arc::new(AsyncRwLock::new(table)),
            ctx: Arc::new(ctx),
            pod_id,
        })
    }

    /// Try to claim a task for exclusive execution.
    ///
    /// Returns `true` if this pod successfully claimed the task.
    /// Returns `false` if:
    /// - Another pod is already processing (and the lock is not stale)
    /// - The task is not yet due (`next_run_at` is in the future)
    /// - A concurrent pod won the Delta OCC race
    ///
    /// This is the distributed equivalent of `SELECT ... FOR UPDATE SKIP LOCKED`.
    pub async fn try_claim_task(&self, task_name: &str) -> Result<bool, TraceEngineError> {
        let mut table_guard = self.table.write().await;

        // Refresh from storage to see commits from other pods
        if let Err(e) = table_guard.update_incremental(None).await {
            debug!("Control table update skipped (new table): {}", e);
        }

        // Re-register so DataFusion sees latest state
        let _ = self.ctx.deregister_table(CONTROL_TABLE_NAME);
        if let Ok(provider) = table_guard.table_provider().await {
            self.ctx.register_table(CONTROL_TABLE_NAME, provider)?;
        }

        // Read current task state
        let current = self.read_task(&table_guard_to_ctx(&self.ctx), task_name).await?;

        let now = Utc::now();

        match current {
            Some(record) => {
                // Check if another pod is actively processing
                if record.status == status::PROCESSING {
                    let stale_threshold = now - Duration::minutes(STALE_LOCK_MINUTES);
                    if record.claimed_at > stale_threshold {
                        debug!(
                            "Task '{}' is being processed by pod '{}' (not stale), skipping",
                            task_name, record.pod_id
                        );
                        return Ok(false);
                    }
                    warn!(
                        "Task '{}' claimed by pod '{}' is stale (claimed_at: {}), reclaiming",
                        task_name, record.pod_id, record.claimed_at
                    );
                }

                // Check if task is due
                if now < record.next_run_at {
                    debug!(
                        "Task '{}' not due until {}, skipping",
                        task_name, record.next_run_at
                    );
                    return Ok(false);
                }

                // Claim the task by overwriting the entire table content for this task.
                // Delta OCC ensures only one pod wins.
                let claimed = TaskRecord {
                    task_name: task_name.to_string(),
                    status: status::PROCESSING.to_string(),
                    pod_id: self.pod_id.clone(),
                    claimed_at: now,
                    completed_at: None,
                    next_run_at: record.next_run_at,
                };

                match self.write_task_update(&mut table_guard, &claimed).await {
                    Ok(()) => {
                        info!("Successfully claimed task '{}'", task_name);
                        Ok(true)
                    }
                    Err(TraceEngineError::DataTableError(ref e))
                        if e.to_string().contains("Transaction") =>
                    {
                        info!(
                            "Lost OCC race for task '{}' to another pod",
                            task_name
                        );
                        Ok(false)
                    }
                    Err(e) => Err(e),
                }
            }
            None => {
                // Task doesn't exist yet — create it in "processing" state.
                // The first pod to reach here wins via OCC.
                let claimed = TaskRecord {
                    task_name: task_name.to_string(),
                    status: status::PROCESSING.to_string(),
                    pod_id: self.pod_id.clone(),
                    claimed_at: now,
                    completed_at: None,
                    next_run_at: now, // Will be updated on release
                };

                match self.write_task_update(&mut table_guard, &claimed).await {
                    Ok(()) => {
                        info!("Created and claimed new task '{}'", task_name);
                        Ok(true)
                    }
                    Err(TraceEngineError::DataTableError(ref e))
                        if e.to_string().contains("Transaction") =>
                    {
                        info!(
                            "Lost OCC race for new task '{}' to another pod",
                            task_name
                        );
                        Ok(false)
                    }
                    Err(e) => Err(e),
                }
            }
        }
    }

    /// Release a task after successful completion, scheduling the next run.
    ///
    /// Sets status to "idle" and updates `next_run_at` based on the provided interval.
    pub async fn release_task(
        &self,
        task_name: &str,
        next_run_interval: Duration,
    ) -> Result<(), TraceEngineError> {
        let mut table_guard = self.table.write().await;
        let now = Utc::now();

        let released = TaskRecord {
            task_name: task_name.to_string(),
            status: status::IDLE.to_string(),
            pod_id: self.pod_id.clone(),
            claimed_at: now,
            completed_at: Some(now),
            next_run_at: now + next_run_interval,
        };

        self.write_task_update(&mut table_guard, &released).await?;

        info!(
            "Released task '{}', next run at {}",
            task_name, released.next_run_at
        );
        Ok(())
    }

    /// Release a task after a failure, keeping the original `next_run_at` so it
    /// can be retried immediately by any pod.
    pub async fn release_task_on_failure(
        &self,
        task_name: &str,
    ) -> Result<(), TraceEngineError> {
        let mut table_guard = self.table.write().await;

        // Refresh to get current next_run_at
        if let Err(e) = table_guard.update_incremental(None).await {
            debug!("Control table update skipped: {}", e);
        }

        let _ = self.ctx.deregister_table(CONTROL_TABLE_NAME);
        if let Ok(provider) = table_guard.table_provider().await {
            self.ctx.register_table(CONTROL_TABLE_NAME, provider)?;
        }

        let current = self
            .read_task(&table_guard_to_ctx(&self.ctx), task_name)
            .await?;

        let now = Utc::now();
        let next_run = current
            .map(|r| r.next_run_at)
            .unwrap_or(now);

        let released = TaskRecord {
            task_name: task_name.to_string(),
            status: status::IDLE.to_string(),
            pod_id: self.pod_id.clone(),
            claimed_at: now,
            completed_at: Some(now),
            next_run_at: next_run,
        };

        self.write_task_update(&mut table_guard, &released).await?;

        warn!(
            "Released task '{}' after failure, next_run_at unchanged: {}",
            task_name, next_run
        );
        Ok(())
    }

    /// Check if a task is due and not currently being processed.
    pub async fn is_task_due(&self, task_name: &str) -> Result<bool, TraceEngineError> {
        let mut table_guard = self.table.write().await;

        if let Err(e) = table_guard.update_incremental(None).await {
            debug!("Control table update skipped: {}", e);
        }

        let _ = self.ctx.deregister_table(CONTROL_TABLE_NAME);
        if let Ok(provider) = table_guard.table_provider().await {
            self.ctx.register_table(CONTROL_TABLE_NAME, provider)?;
        }

        let current = self
            .read_task(&table_guard_to_ctx(&self.ctx), task_name)
            .await?;

        let now = Utc::now();
        match current {
            Some(record) => {
                if record.status == status::PROCESSING {
                    let stale_threshold = now - Duration::minutes(STALE_LOCK_MINUTES);
                    // Due only if the lock is stale
                    Ok(record.claimed_at <= stale_threshold)
                } else {
                    Ok(now >= record.next_run_at)
                }
            }
            // Never registered = due (first run)
            None => Ok(true),
        }
    }

    /// Read a single task record from the control table via DataFusion.
    async fn read_task(
        &self,
        ctx: &SessionContext,
        task_name: &str,
    ) -> Result<Option<TaskRecord>, TraceEngineError> {
        let table_exists = ctx.table_exist(CONTROL_TABLE_NAME)?;
        if !table_exists {
            return Ok(None);
        }

        let df = ctx
            .table(CONTROL_TABLE_NAME)
            .await
            .map_err(TraceEngineError::DatafusionError)?;

        let df = df
            .filter(col("task_name").eq(lit(task_name)))
            .map_err(TraceEngineError::DatafusionError)?;

        let batches = df
            .collect()
            .await
            .map_err(TraceEngineError::DatafusionError)?;

        // Extract the first (and only) row if it exists.
        // DataFusion may return Utf8View instead of Utf8 for string columns,
        // so cast to Utf8 before downcast to StringArray.
        for batch in &batches {
            if batch.num_rows() == 0 {
                continue;
            }

            let get_string = |col_name: &str| -> String {
                let col = batch.column_by_name(col_name).unwrap();
                let casted =
                    arrow::compute::cast(col, &DataType::Utf8).expect("cast to Utf8 failed");
                let arr = casted.as_any().downcast_ref::<StringArray>().unwrap();
                arr.value(0).to_string()
            };

            let get_timestamp = |col_name: &str| -> Option<DateTime<Utc>> {
                let col = batch.column_by_name(col_name).unwrap();
                if col.is_null(0) {
                    return None;
                }
                let arr = col
                    .as_any()
                    .downcast_ref::<TimestampMicrosecondArray>()
                    .unwrap();
                DateTime::from_timestamp_micros(arr.value(0))
            };

            let task_name_val = get_string("task_name");
            let status_val = get_string("status");
            let pod_id_val = get_string("pod_id");
            let claimed_at = get_timestamp("claimed_at").unwrap_or_else(Utc::now);
            let completed_at = get_timestamp("completed_at");
            let next_run_at = get_timestamp("next_run_at").unwrap_or_else(Utc::now);

            return Ok(Some(TaskRecord {
                task_name: task_name_val,
                status: status_val,
                pod_id: pod_id_val,
                claimed_at,
                completed_at,
                next_run_at,
            }));
        }

        Ok(None)
    }

    /// Write a task update using Delta Lake MERGE-like semantics.
    ///
    /// Strategy: DELETE the existing row for this task_name, then APPEND the new row.
    /// Delta OCC ensures atomicity — if two pods race, one commit fails with
    /// `TransactionError` and the caller can retry or back off.
    async fn write_task_update(
        &self,
        table_guard: &mut DeltaTable,
        record: &TaskRecord,
    ) -> Result<(), TraceEngineError> {
        let batch = build_task_batch(&self.schema, record)?;

        // First, delete the existing row for this task (if any).
        // On a brand-new table with no data, delete will fail — that's fine.
        let predicate = format!("task_name = '{}'", record.task_name);
        let delete_result = table_guard.clone().delete().with_predicate(predicate).await;

        match delete_result {
            Ok((updated_table, _metrics)) => {
                // Delete succeeded — now append the new row to the updated table
                let updated_table = updated_table
                    .write(vec![batch])
                    .with_save_mode(deltalake::protocol::SaveMode::Append)
                    .await?;

                let _ = self.ctx.deregister_table(CONTROL_TABLE_NAME);
                if let Ok(provider) = updated_table.table_provider().await {
                    self.ctx.register_table(CONTROL_TABLE_NAME, provider)?;
                }
                *table_guard = updated_table;
            }
            Err(_) => {
                // No existing data to delete (new table) — just append
                let updated_table = table_guard
                    .clone()
                    .write(vec![batch])
                    .with_save_mode(deltalake::protocol::SaveMode::Append)
                    .await?;

                let _ = self.ctx.deregister_table(CONTROL_TABLE_NAME);
                if let Ok(provider) = updated_table.table_provider().await {
                    self.ctx.register_table(CONTROL_TABLE_NAME, provider)?;
                }
                *table_guard = updated_table;
            }
        }

        Ok(())
    }
}

/// Helper to avoid borrow issues — just returns the ctx reference.
fn table_guard_to_ctx(ctx: &Arc<SessionContext>) -> SessionContext {
    ctx.as_ref().clone()
}

/// Build or load the control table at `{base_url}/_scouter_control/`.
async fn build_or_create_control_table(
    object_store: &ObjectStore,
    schema: SchemaRef,
) -> Result<DeltaTable, TraceEngineError> {
    // Reuse the cloud logstore factories registered by the trace engine.
    // Safe to call repeatedly — existing entries are not overwritten.
    register_cloud_logstore_factories();

    let base_url = object_store.get_base_url()?;
    let control_url = append_path_to_url(&base_url, CONTROL_TABLE_NAME)?;

    info!("Loading control table at URL: {}", control_url);

    let store = object_store.as_dyn_object_store();

    let is_delta_table = if control_url.scheme() == "file" {
        if let Ok(path) = control_url.to_file_path() {
            if !path.exists() {
                info!("Creating directory for control table: {:?}", path);
                std::fs::create_dir_all(&path)?;
            }
            path.join("_delta_log").exists()
        } else {
            false
        }
    } else {
        match DeltaTableBuilder::from_url(control_url.clone()) {
            Ok(builder) => builder
                .with_storage_backend(store.clone(), control_url.clone())
                .load()
                .await
                .is_ok(),
            Err(_) => false,
        }
    };

    if is_delta_table {
        info!("Loading existing control table");
        let table = DeltaTableBuilder::from_url(control_url.clone())?
            .with_storage_backend(store, control_url)
            .load()
            .await?;
        Ok(table)
    } else {
        info!("Creating new control table");
        let table = DeltaTableBuilder::from_url(control_url.clone())?
            .with_storage_backend(store, control_url)
            .build()?;

        let delta_fields = arrow_schema_to_delta(&schema);

        table
            .create()
            .with_table_name(CONTROL_TABLE_NAME)
            .with_columns(delta_fields)
            .with_configuration_property(TableProperty::CheckpointInterval, Some("5"))
            .await
            .map_err(Into::into)
    }
}

/// Register cloud logstore factories (mirrors the trace engine's registration).
fn register_cloud_logstore_factories() {
    use deltalake::logstore::{
        default_logstore, LogStore, LogStoreFactory, ObjectStoreRef, StorageConfig,
    };

    struct PassthroughLogStoreFactory;

    impl LogStoreFactory for PassthroughLogStoreFactory {
        fn with_options(
            &self,
            prefixed_store: ObjectStoreRef,
            root_store: ObjectStoreRef,
            location: &Url,
            options: &StorageConfig,
        ) -> deltalake::DeltaResult<Arc<dyn LogStore>> {
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

    let factories = logstore_factories();
    let factory = Arc::new(PassthroughLogStoreFactory) as Arc<dyn LogStoreFactory>;
    for scheme in ["gs", "s3", "s3a", "az", "abfs", "abfss"] {
        let key = Url::parse(&format!("{}://", scheme)).expect("scheme is a valid URL prefix");
        if !factories.contains_key(&key) {
            factories.insert(key, factory.clone());
        }
    }
}

/// Append a path segment to a URL, handling trailing slashes correctly.
fn append_path_to_url(base: &Url, segment: &str) -> Result<Url, TraceEngineError> {
    let mut url = base.clone();
    // Ensure the base path ends with '/'
    if !url.path().ends_with('/') {
        url.set_path(&format!("{}/", url.path()));
    }
    url = url.join(segment)?;
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use scouter_settings::ObjectStorageSettings;

    fn cleanup() {
        let storage_settings = ObjectStorageSettings::default();
        let current_dir = std::env::current_dir().unwrap();
        let storage_path = current_dir.join(storage_settings.storage_root());
        if storage_path.exists() {
            std::fs::remove_dir_all(storage_path).unwrap();
        }
    }

    #[tokio::test]
    async fn test_control_table_init() -> Result<(), TraceEngineError> {
        cleanup();

        let settings = ObjectStorageSettings::default();
        let engine = ControlTableEngine::new(&settings, "pod-1".to_string()).await?;

        // No tasks should exist yet
        let due = engine.is_task_due("optimize").await?;
        assert!(due, "New task should be due (never run before)");

        cleanup();
        Ok(())
    }

    #[tokio::test]
    async fn test_claim_and_release() -> Result<(), TraceEngineError> {
        cleanup();

        let settings = ObjectStorageSettings::default();
        let engine = ControlTableEngine::new(&settings, "pod-1".to_string()).await?;

        // Claim a new task
        let claimed = engine.try_claim_task("optimize").await?;
        assert!(claimed, "First claim should succeed");

        // Second claim from same engine should fail (task is processing)
        let claimed_again = engine.try_claim_task("optimize").await?;
        assert!(!claimed_again, "Second claim should fail (already processing)");

        // Release with 1-hour interval
        engine
            .release_task("optimize", Duration::hours(1))
            .await?;

        // Task should not be due yet (next_run_at is 1 hour from now)
        let due = engine.is_task_due("optimize").await?;
        assert!(!due, "Task should not be due yet");

        cleanup();
        Ok(())
    }

    #[tokio::test]
    async fn test_claim_release_then_due() -> Result<(), TraceEngineError> {
        cleanup();

        let settings = ObjectStorageSettings::default();
        let engine = ControlTableEngine::new(&settings, "pod-1".to_string()).await?;

        // Claim and release with 0-second interval (immediately due again)
        let claimed = engine.try_claim_task("vacuum").await?;
        assert!(claimed);

        engine
            .release_task("vacuum", Duration::seconds(0))
            .await?;

        // Should be due now
        let due = engine.is_task_due("vacuum").await?;
        assert!(due, "Task should be due after 0-second interval");

        // Should be claimable again
        let claimed = engine.try_claim_task("vacuum").await?;
        assert!(claimed, "Task should be claimable after release");

        // Release on failure — next_run_at stays the same
        engine.release_task_on_failure("vacuum").await?;

        cleanup();
        Ok(())
    }

    #[tokio::test]
    async fn test_multiple_tasks() -> Result<(), TraceEngineError> {
        cleanup();

        let settings = ObjectStorageSettings::default();
        let engine = ControlTableEngine::new(&settings, "pod-1".to_string()).await?;

        // Claim two different tasks
        let claimed_opt = engine.try_claim_task("optimize").await?;
        let claimed_vac = engine.try_claim_task("vacuum").await?;
        assert!(claimed_opt, "Optimize claim should succeed");
        assert!(claimed_vac, "Vacuum claim should succeed");

        // Release both
        engine
            .release_task("optimize", Duration::hours(24))
            .await?;
        engine
            .release_task("vacuum", Duration::hours(168))
            .await?;

        cleanup();
        Ok(())
    }
}
