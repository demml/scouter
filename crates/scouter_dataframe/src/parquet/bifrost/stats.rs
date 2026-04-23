use crate::error::DatasetEngineError;
use crate::parquet::utils::register_cloud_logstore_factories;
use crate::storage::ObjectStore;
use deltalake::DeltaTableBuilder;
use scouter_types::dataset::DatasetNamespace;
use tracing::debug;
use url::Url;

/// Table-level statistics extracted from the Delta Lake transaction log.
/// No data files are read — only the log metadata.
#[derive(Debug, Clone, Default)]
pub struct TableStats {
    pub row_count: Option<u64>,
    pub file_count: Option<u64>,
    pub size_bytes: Option<u64>,
    pub delta_version: Option<u64>,
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

/// Load stats from a Delta table's transaction log without scanning data.
///
/// For inactive tables (no engine loaded), this transiently loads the Delta log.
/// For active tables, prefer reading from the engine's `DeltaTable` directly.
pub async fn load_table_stats(
    object_store: &ObjectStore,
    namespace: &DatasetNamespace,
) -> Result<TableStats, DatasetEngineError> {
    register_cloud_logstore_factories();
    let table_url = build_table_url(object_store, namespace)?;

    let store = object_store.as_dyn_object_store();
    let builder =
        DeltaTableBuilder::from_url(table_url.clone())?.with_storage_backend(store, table_url);

    let table = match builder.load().await {
        Ok(t) => t,
        Err(e) => {
            // Only treat "table doesn't exist yet" as empty stats
            let msg = e.to_string().to_lowercase();
            if msg.contains("not a delta table")
                || msg.contains("no such file")
                || msg.contains("does not exist")
            {
                return Ok(TableStats::default());
            }
            return Err(DatasetEngineError::DeltaTableError(e));
        }
    };

    extract_stats_from_snapshot(&table)
}

/// Extract stats from a loaded DeltaTable's snapshot.
pub fn extract_stats_from_snapshot(
    table: &deltalake::DeltaTable,
) -> Result<TableStats, DatasetEngineError> {
    let snapshot = table.snapshot()?;
    let version = snapshot.version();
    let log_data = snapshot.log_data();

    let file_count = log_data.num_files() as u64;
    let mut row_count: u64 = 0;
    let mut size_bytes: u64 = 0;
    let mut has_row_stats = false;

    for file_view in log_data.iter() {
        size_bytes = size_bytes.saturating_add(file_view.size().max(0) as u64);
        if let Some(n) = file_view.num_records() {
            row_count += n as u64;
            has_row_stats = true;
        }
    }

    debug!(
        "Delta stats: version={}, files={}, size={}B, rows={}",
        version,
        file_count,
        size_bytes,
        if has_row_stats {
            row_count.to_string()
        } else {
            "unknown".to_string()
        }
    );

    Ok(TableStats {
        row_count: if has_row_stats { Some(row_count) } else { None },
        file_count: Some(file_count),
        size_bytes: Some(size_bytes),
        delta_version: Some(version),
    })
}
