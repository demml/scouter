use crate::caching_store::CachingStore;
use crate::error::StorageError;
use base64::prelude::*;
use datafusion::prelude::{SessionConfig, SessionContext};
use futures::TryStreamExt;
use object_store::aws::{AmazonS3, AmazonS3Builder};
use object_store::azure::{MicrosoftAzure, MicrosoftAzureBuilder};
use object_store::gcp::{GoogleCloudStorage, GoogleCloudStorageBuilder};
use object_store::local::LocalFileSystem;
use object_store::path::Path;
use object_store::ClientOptions;
use object_store::ObjectStore as ObjStore;
use scouter_settings::ObjectStorageSettings;
use scouter_types::StorageType;
use std::sync::Arc;
use tracing::debug;
use url::Url;

/// HTTP client options for cloud object stores.
///
/// Enables TCP+TLS connection pooling so repeat queries reuse existing
/// connections. Sized for high-concurrency GCS/S3 workloads where many
/// parallel readers share the same host.
fn cloud_client_options() -> ClientOptions {
    ClientOptions::new()
        .with_pool_idle_timeout(std::time::Duration::from_secs(120))
        .with_pool_max_idle_per_host(64)
        .with_timeout(std::time::Duration::from_secs(30))
        .with_connect_timeout(std::time::Duration::from_secs(5))
}

/// Helper function to decode base64 encoded string
fn decode_base64_str(service_base64_creds: &str) -> Result<String, StorageError> {
    let decoded = BASE64_STANDARD.decode(service_base64_creds)?;

    Ok(String::from_utf8(decoded)?)
}

/// Storage provider enum for common object stores
#[derive(Debug, Clone)]
enum StorageProvider {
    Google(Arc<CachingStore<GoogleCloudStorage>>),
    Aws(Arc<CachingStore<AmazonS3>>),
    Local(Arc<CachingStore<LocalFileSystem>>),
    Azure(Arc<CachingStore<MicrosoftAzure>>),
}

impl StorageProvider {
    /// Return the inner object store as a type-erased `Arc<dyn ObjectStore>`.
    ///
    /// Used by the Delta Lake engine to bypass the storage factory via
    /// `DeltaTableBuilder::with_storage_backend` — necessary for cloud stores
    /// (GCS, S3, Azure) whose schemes are not registered in the default factory.
    pub fn as_dyn_object_store(&self) -> Arc<dyn ObjStore> {
        match self {
            StorageProvider::Google(s) => s.clone() as Arc<dyn ObjStore>,
            StorageProvider::Aws(s) => s.clone() as Arc<dyn ObjStore>,
            StorageProvider::Local(s) => s.clone() as Arc<dyn ObjStore>,
            StorageProvider::Azure(s) => s.clone() as Arc<dyn ObjStore>,
        }
    }

    pub fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, StorageError> {
        let cache_bytes = storage_settings.object_cache_mb() * 1024 * 1024;

        let store = match storage_settings.storage_type {
            StorageType::Google => {
                let mut builder = GoogleCloudStorageBuilder::from_env();

                // Try to use base64 credentials if available
                if let Ok(base64_creds) = std::env::var("GOOGLE_ACCOUNT_JSON_BASE64") {
                    let key = decode_base64_str(&base64_creds)?;
                    builder = builder.with_service_account_key(&key);
                    debug!("Using base64 encoded service account key for Google Cloud Storage");
                }

                // Add bucket name and build
                let storage = builder
                    .with_bucket_name(storage_settings.storage_root())
                    .with_client_options(cloud_client_options())
                    .build()?;

                StorageProvider::Google(Arc::new(CachingStore::new(storage, cache_bytes)))
            }
            StorageType::Aws => {
                let storage = AmazonS3Builder::from_env()
                    .with_bucket_name(storage_settings.storage_root())
                    .with_region(storage_settings.region.clone())
                    .with_client_options(cloud_client_options())
                    .build()?;
                StorageProvider::Aws(Arc::new(CachingStore::new(storage, cache_bytes)))
            }
            StorageType::Local => {
                let storage = LocalFileSystem::new();
                StorageProvider::Local(Arc::new(CachingStore::new(storage, cache_bytes)))
            }
            StorageType::Azure => {
                // MicrosoftAzureBuilder::from_env() reads AZURE_STORAGE_ACCOUNT_NAME
                // and AZURE_STORAGE_ACCOUNT_KEY specifically.  Many Azure tools
                // (az CLI, Terraform, GitHub Actions) emit AZURE_STORAGE_ACCOUNT and
                // AZURE_STORAGE_KEY instead.  Accept both so callers don't need to
                // know which naming convention object_store expects.
                let mut builder = MicrosoftAzureBuilder::from_env();

                if std::env::var("AZURE_STORAGE_ACCOUNT_NAME").is_err() {
                    if let Ok(account) = std::env::var("AZURE_STORAGE_ACCOUNT") {
                        builder = builder.with_account(account);
                    }
                }
                if std::env::var("AZURE_STORAGE_ACCOUNT_KEY").is_err() {
                    if let Ok(key) = std::env::var("AZURE_STORAGE_KEY") {
                        builder = builder.with_access_key(key);
                    }
                }

                let storage = builder
                    .with_container_name(storage_settings.storage_root())
                    .with_client_options(cloud_client_options())
                    .build()?;

                StorageProvider::Azure(Arc::new(CachingStore::new(storage, cache_bytes)))
            }
        };

        Ok(store)
    }

    pub fn get_base_url(
        &self,
        storage_settings: &ObjectStorageSettings,
    ) -> Result<Url, StorageError> {
        match self {
            StorageProvider::Google(_) => Ok(Url::parse(&storage_settings.storage_uri)?),
            StorageProvider::Aws(_) => Ok(Url::parse(&storage_settings.storage_uri)?),
            StorageProvider::Local(_) => {
                // Convert relative path to absolute path for local filesystem
                let storage_path = std::path::PathBuf::from(storage_settings.storage_root());
                let absolute_path = if storage_path.is_absolute() {
                    storage_path
                } else {
                    std::env::current_dir()?.join(storage_path)
                };

                // Create file:// URL with absolute path
                let url = Url::from_file_path(&absolute_path).map_err(|_| {
                    StorageError::InvalidUrl(format!(
                        "Failed to create file URL from path: {:?}",
                        absolute_path
                    ))
                })?;
                Ok(url)
            }
            StorageProvider::Azure(_) => Ok(Url::parse(&storage_settings.storage_uri)?),
        }
    }

    pub fn get_session(
        &self,
        storage_settings: &ObjectStorageSettings,
    ) -> Result<SessionContext, StorageError> {
        let mut config = SessionConfig::new()
            .with_target_partitions(
                std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(4),
            )
            .with_batch_size(8192)
            .with_prefer_existing_sort(true)
            .with_parquet_pruning(true)
            .with_collect_statistics(true);

        // Push filter predicates into the Parquet reader so only matching rows are decoded,
        // and reorder predicates by selectivity so bloom filters (trace_id, entity_id) are
        // evaluated before range checks (start_time), short-circuiting row evaluation early.
        config.options_mut().execution.parquet.pushdown_filters = true;
        config.options_mut().execution.parquet.reorder_filters = true;

        // ── Parquet read-path tuning (GCS latency reduction) ──────────────
        //
        // Read at least 1MB from the end of each Parquet file in a single request.
        // Default is 512KB. Our files have bloom filters on trace_id + entity_id
        // and page-level statistics on start_time + status_code, so footers are
        // larger than average. 1MB captures footer + column/offset indexes in one
        // GCS round-trip instead of the default multi-step chain, saving 1-2
        // round-trips (~30-60ms each) per file.
        config
            .options_mut()
            .execution
            .parquet
            .metadata_size_hint = Some(1024 * 1024);

        // Bloom filters are written on trace_id and entity_id — ensure the reader
        // consults them before decoding row groups. (Default is true in DF 52, but
        // we're explicit to guard against version changes.)
        config
            .options_mut()
            .execution
            .parquet
            .bloom_filter_on_read = true;

        // Read Utf8 columns as Utf8View and Binary as BinaryView for zero-copy.
        // Our schema already uses Utf8View/BinaryView — this ensures DataFusion
        // doesn't downgrade them when reading back from Parquet.
        config
            .options_mut()
            .execution
            .parquet
            .schema_force_view_types = true;

        // ── Listing / metadata concurrency ───────────────────────────────
        //
        // Number of files to stat in parallel when inferring schema or listing
        // a Delta table's backing Parquet files. Default is 32. On GCS each
        // stat is a separate HTTP HEAD; higher concurrency hides the per-file
        // latency behind parallelism. 64 matches our pool_max_idle_per_host.
        config
            .options_mut()
            .execution
            .meta_fetch_concurrency = 64;

        // ── Write-path tuning ────────────────────────────────────────────
        //
        // Increase write-side parallelism so compaction and flush can encode
        // multiple row groups concurrently, reducing wall-clock write latency.
        config
            .options_mut()
            .execution
            .parquet
            .maximum_parallel_row_group_writers = 4;

        // Buffer more decoded record batches per stream before back-pressure
        // kicks in, smoothing out bursty reads from GCS.
        config
            .options_mut()
            .execution
            .parquet
            .maximum_buffered_record_batches_per_stream = 8;

        let ctx = SessionContext::new_with_config(config);
        let base_url = self.get_base_url(storage_settings)?;

        match self {
            StorageProvider::Google(store) => {
                ctx.register_object_store(&base_url, store.clone());
            }
            StorageProvider::Aws(store) => {
                ctx.register_object_store(&base_url, store.clone());
            }
            StorageProvider::Local(store) => {
                ctx.register_object_store(&base_url, store.clone());
            }
            StorageProvider::Azure(store) => {
                ctx.register_object_store(&base_url, store.clone());
            }
        }

        Ok(ctx)
    }

    /// List files in the object store
    ///
    /// # Arguments
    /// * `path` - The path to list files from. If None, lists all files in the root.
    ///
    /// # Returns
    /// * `Result<Vec<String>, StorageError>` - A result containing a vector of file paths or an error.
    pub async fn list(&self, path: Option<&Path>) -> Result<Vec<String>, StorageError> {
        let stream = match self {
            StorageProvider::Local(store) => store.list(path),
            StorageProvider::Google(store) => store.list(path),
            StorageProvider::Aws(store) => store.list(path),
            StorageProvider::Azure(store) => store.list(path),
        };

        // Process each item in the stream
        stream
            .try_fold(Vec::new(), |mut files, meta| async move {
                files.push(meta.location.to_string());
                Ok(files)
            })
            .await
            .map_err(Into::into)
    }

    pub async fn delete(&self, path: &Path) -> Result<(), StorageError> {
        match self {
            StorageProvider::Local(store) => {
                store.delete(path).await?;
                Ok(())
            }
            StorageProvider::Google(store) => {
                store.delete(path).await?;
                Ok(())
            }
            StorageProvider::Aws(store) => {
                store.delete(path).await?;
                Ok(())
            }
            StorageProvider::Azure(store) => {
                store.delete(path).await?;
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ObjectStore {
    provider: StorageProvider,
    pub storage_settings: ObjectStorageSettings,
}

impl ObjectStore {
    /// Creates a new ObjectStore instance.
    ///
    /// # Arguments
    /// * `storage_settings` - The settings for the object storage.
    ///
    /// # Returns
    /// * `Result<ObjectStore, StorageError>` - A result containing the ObjectStore instance or an error.
    pub fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, StorageError> {
        let store = StorageProvider::new(storage_settings)?;
        Ok(ObjectStore {
            provider: store,
            storage_settings: storage_settings.clone(),
        })
    }

    pub fn get_session(&self) -> Result<SessionContext, StorageError> {
        let ctx = self.provider.get_session(&self.storage_settings)?;
        Ok(ctx)
    }

    /// Return the inner object store as a type-erased `Arc<dyn ObjectStore>`.
    ///
    /// Pass this to `DeltaTableBuilder::with_storage_backend` to bypass the Delta Lake
    /// storage factory (required for GCS, S3, and Azure).
    pub fn as_dyn_object_store(&self) -> Arc<dyn ObjStore> {
        self.provider.as_dyn_object_store()
    }

    /// Get the base URL for datafusion to use
    pub fn get_base_url(&self) -> Result<Url, StorageError> {
        self.provider.get_base_url(&self.storage_settings)
    }

    /// List files in the object store
    ///
    /// When path is None, lists from the root.
    /// When path is provided, lists from that path.
    ///
    /// Note: The path parameter should NOT include the storage root - it's a relative path
    /// that will be automatically combined with the storage root.
    pub async fn list(&self, path: Option<&Path>) -> Result<Vec<String>, StorageError> {
        self.provider.list(path).await
    }

    pub async fn delete(&self, path: &Path) -> Result<(), StorageError> {
        self.provider.delete(path).await
    }
}
