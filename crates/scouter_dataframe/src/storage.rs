use std::sync::Arc;

use datafusion::prelude::SessionContext;
use futures::stream::BoxStream;
use futures::StreamExt;
use object_store::aws::{AmazonS3, AmazonS3Builder};
use object_store::azure::{MicrosoftAzure, MicrosoftAzureBuilder};
use object_store::gcp::{GoogleCloudStorage, GoogleCloudStorageBuilder};
use object_store::local::LocalFileSystem;
use object_store::path::Path;
use object_store::ObjectStore as ObjStore;
use scouter_error::StorageError;
use scouter_settings::ObjectStorageSettings;
use scouter_types::StorageType;
use std::path::Path as StdPath;
use url::Url;

/// Storage provider enum for common object stores
#[derive(Debug, Clone)]
enum StorageProvider {
    Google(Arc<GoogleCloudStorage>),
    Aws(Arc<AmazonS3>),
    Local(Arc<LocalFileSystem>),
    Azure(Arc<MicrosoftAzure>),
}

impl StorageProvider {
    pub fn new(storage_settings: &ObjectStorageSettings) -> Result<Self, StorageError> {
        let store = match storage_settings.storage_type {
            StorageType::Google => {
                let builder = GoogleCloudStorageBuilder::from_env()
                    .with_bucket_name(storage_settings.storage_root())
                    .build()
                    .map_err(|_| {
                        StorageError::ObjectStoreError(
                            "Failed to create Google Cloud Storage builder".to_string(),
                        )
                    })?;

                StorageProvider::Google(Arc::new(builder))
            }
            StorageType::Aws => {
                let builder = AmazonS3Builder::from_env()
                    .with_bucket_name(storage_settings.storage_root())
                    .with_region(storage_settings.region.clone())
                    .build()
                    .map_err(|_| {
                        StorageError::ObjectStoreError(
                            "Failed to create AWS S3 builder".to_string(),
                        )
                    })?;
                StorageProvider::Aws(Arc::new(builder))
            }
            StorageType::Local => {
                // Create LocalFileSystem with the root path as the prefix
                let builder = LocalFileSystem::new();
                StorageProvider::Local(Arc::new(builder))
            }
            StorageType::Azure => {
                let builder = MicrosoftAzureBuilder::from_env()
                    .with_container_name(storage_settings.storage_root())
                    .build()
                    .map_err(|_| {
                        StorageError::ObjectStoreError(
                            "Failed to create Azure file system builder".to_string(),
                        )
                    })?;

                StorageProvider::Azure(Arc::new(builder))
            }
        };

        Ok(store)
    }

    pub fn get_base_url(
        &self,
        storage_settings: &ObjectStorageSettings,
    ) -> Result<Url, StorageError> {
        match self {
            StorageProvider::Google(_) => Url::parse(&storage_settings.storage_uri).map_err(|_| {
                StorageError::ObjectStoreError(
                    "Failed to parse Google Cloud Storage URI".to_string(),
                )
            }),
            StorageProvider::Aws(_) => Url::parse(&storage_settings.storage_uri).map_err(|_| {
                StorageError::ObjectStoreError("Failed to parse AWS S3 URI".to_string())
            }),
            StorageProvider::Local(_) => {
                // For local storage, use a file:// URL scheme
                Url::parse("file:///").map_err(|_| {
                    StorageError::ObjectStoreError(
                        "Failed to parse local file system URI".to_string(),
                    )
                })
            }
            StorageProvider::Azure(_) => Url::parse(&storage_settings.storage_uri).map_err(|_| {
                StorageError::ObjectStoreError("Failed to parse Azure file system URI".to_string())
            }),
        }
    }

    pub fn get_session(
        &self,
        storage_settings: &ObjectStorageSettings,
    ) -> Result<SessionContext, StorageError> {
        let ctx = SessionContext::new();
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
        let mut files = Vec::new();

        // Get the stream based on the provided path
        let mut stream = match self {
            StorageProvider::Local(store) => store.list(path),
            StorageProvider::Google(store) => store.list(path),
            StorageProvider::Aws(store) => store.list(path),
            StorageProvider::Azure(store) => store.list(path),
        };

        // Process each item in the stream
        while let Some(result) = stream.next().await {
            match result {
                Ok(meta) => {
                    files.push(meta.location.to_string());
                }
                Err(e) => {
                    return Err(StorageError::ObjectStoreError(format!(
                        "Error listing files: {}",
                        e
                    )));
                }
            }
        }

        Ok(files)
    }

    pub async fn delete(&self, path: &Path) -> Result<(), StorageError> {
        match self {
            StorageProvider::Local(store) => {
                store.delete(path).await.map_err(|e| {
                    StorageError::ObjectStoreError(format!("Failed to delete file: {}", e))
                })?;
                Ok(())
            }
            StorageProvider::Google(store) => {
                store.delete(path).await.map_err(|e| {
                    StorageError::ObjectStoreError(format!("Failed to delete file: {}", e))
                })?;
                Ok(())
            }
            StorageProvider::Aws(store) => {
                store.delete(path).await.map_err(|e| {
                    StorageError::ObjectStoreError(format!("Failed to delete file: {}", e))
                })?;
                Ok(())
            }
            StorageProvider::Azure(store) => {
                store.delete(path).await.map_err(|e| {
                    StorageError::ObjectStoreError(format!("Failed to delete file: {}", e))
                })?;
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
