use crate::ScouterServerConfig;
use scouter_types::StorageType;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct ObjectStorageSettings {
    pub storage_uri: String,
    pub storage_type: StorageType,
    pub region: String, // this is aws specific
    /// How often the Delta Lake compaction (Z-ORDER optimize) runs for trace tables. Default: 24h.
    pub trace_compaction_interval_hours: u64,
    /// How often the span buffer flushes to Delta Lake. Default: 5s.
    pub trace_flush_interval_secs: u64,
}

impl Default for ObjectStorageSettings {
    fn default() -> Self {
        let storage_uri = std::env::var("SCOUTER_STORAGE_URI")
            .unwrap_or_else(|_| "./scouter_storage".to_string());

        let storage_type = ScouterServerConfig::get_storage_type(&storage_uri);

        // need to set this for aws objectstore
        let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());

        let trace_compaction_interval_hours =
            std::env::var("SCOUTER_TRACE_COMPACTION_INTERVAL_HOURS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(24u64);

        let trace_flush_interval_secs = std::env::var("SCOUTER_TRACE_FLUSH_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5u64);

        Self {
            storage_uri,
            storage_type,
            region,
            trace_compaction_interval_hours,
            trace_flush_interval_secs,
        }
    }
}

impl ObjectStorageSettings {
    /// Buffer size for trace span batching before flushing to Delta Lake.
    ///
    /// Configurable via `SCOUTER_TRACE_BUFFER_SIZE`. Larger values produce fewer,
    /// bigger Parquet files — reducing Delta log replay cost and file-open overhead
    /// on cloud storage at the expense of slightly longer flush intervals.
    /// Default: 10,000 spans.
    pub fn trace_buffer_size(&self) -> usize {
        std::env::var("SCOUTER_TRACE_BUFFER_SIZE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10_000)
    }

    pub fn storage_root(&self) -> String {
        match self.storage_type {
            StorageType::Google | StorageType::Aws | StorageType::Azure => {
                if let Some(stripped) = self.storage_uri.strip_prefix("gs://") {
                    stripped.split('/').next().unwrap_or(stripped).to_string()
                } else if let Some(stripped) = self.storage_uri.strip_prefix("s3://") {
                    stripped.split('/').next().unwrap_or(stripped).to_string()
                } else if let Some(stripped) = self.storage_uri.strip_prefix("az://") {
                    stripped.split('/').next().unwrap_or(stripped).to_string()
                } else {
                    self.storage_uri.clone()
                }
            }
            StorageType::Local => {
                // For local storage, just return the path directly
                self.storage_uri.clone()
            }
        }
    }

    pub fn canonicalized_path(&self) -> String {
        // if registry is local canonicalize the path
        if self.storage_type == StorageType::Local {
            let path = PathBuf::from(&self.storage_uri);
            if path.exists() {
                path.canonicalize()
                    .unwrap_or_else(|_| path.clone())
                    .to_str()
                    .unwrap()
                    .to_string()
            } else {
                self.storage_uri.clone()
            }
        } else {
            self.storage_uri.clone()
        }
    }
}
