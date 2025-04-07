use std::sync::Arc;

use datafusion::prelude::SessionContext;
use futures::StreamExt;
use object_store::aws::{AmazonS3, AmazonS3Builder};
use object_store::azure::{MicrosoftAzure, MicrosoftAzureBuilder};
use object_store::gcp::{GoogleCloudStorage, GoogleCloudStorageBuilder};
use object_store::local::LocalFileSystem;
use object_store::path::Path;
use object_store::ObjectStore as ObjStore;
use scouter_error::StorageError;
use scouter_settings::{ObjectStorageSettings, StorageType};
use url::Url;

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
                let root = storage_settings.storage_root();

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

    pub fn get_session(
        &self,
        storage_settings: &ObjectStorageSettings,
    ) -> Result<SessionContext, StorageError> {
        let ctx = SessionContext::new();

        match self {
            StorageProvider::Google(store) => {
                let url = Url::parse(&storage_settings.storage_uri).map_err(|_| {
                    StorageError::ObjectStoreError(
                        "Failed to parse Google Cloud Storage URI".to_string(),
                    )
                })?;

                ctx.register_object_store(&url, store.clone());
            }
            StorageProvider::Aws(store) => {
                let url = Url::parse(&storage_settings.storage_uri).map_err(|_| {
                    StorageError::ObjectStoreError("Failed to parse AWS S3 URI".to_string())
                })?;

                ctx.register_object_store(&url, store.clone());
            }
            StorageProvider::Local(store) => {
                // For local storage, use a simple file:// URL scheme
                let url = Url::parse("file:///").map_err(|_| {
                    StorageError::ObjectStoreError(
                        "Failed to parse local file system URI".to_string(),
                    )
                })?;

                ctx.register_object_store(&url, store.clone());
            }
            StorageProvider::Azure(store) => {
                let url = Url::parse(&storage_settings.storage_uri).map_err(|_| {
                    StorageError::ObjectStoreError(
                        "Failed to parse Azure file system URI".to_string(),
                    )
                })?;

                ctx.register_object_store(&url, store.clone());
            }
        }

        Ok(ctx)
    }

    pub async fn list(&self, path: &Path) -> Result<Vec<String>, StorageError> {
        let result = match &self {
            StorageProvider::Local(store) => {
                // canonicalize the path to ensure it is absolute

                let mut files = Vec::new();
                // Get the stream of metadata objects
                let mut list_stream = store.list(Some(path));

                // Process each item in the stream
                while let Some(result) = list_stream.next().await {
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
            StorageProvider::Google(store) => {
                let mut files = Vec::new();
                let mut list_stream = store.list(Some(path));

                while let Some(result) = list_stream.next().await {
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
            // Similar implementations for other providers
            StorageProvider::Aws(store) => {
                let mut files = Vec::new();
                let mut list_stream = store.list(Some(path));

                while let Some(result) = list_stream.next().await {
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
            StorageProvider::Azure(store) => {
                let mut files = Vec::new();
                let mut list_stream = store.list(Some(path));

                while let Some(result) = list_stream.next().await {
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
        };

        result
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

    pub async fn list(&self, path: Option<&Path>) -> Result<Vec<String>, StorageError> {
        let path_to_use = match path {
            Some(p) => Path::from(format!(
                "{}/{}",
                self.storage_settings.canonicalized_path(),
                p.to_string()
            )),
            None => Path::from(self.storage_settings.canonicalized_path()),
        };

        self.provider.list(&path_to_use).await
    }

    pub async fn delete(&self, path: &Path) -> Result<(), StorageError> {
        self.provider.delete(path).await
    }
}
