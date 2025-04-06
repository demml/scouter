use std::sync::Arc;

use datafusion::prelude::SessionContext;
use object_store::aws::{AmazonS3, AmazonS3Builder};
use object_store::azure::{MicrosoftAzure, MicrosoftAzureBuilder};
use object_store::gcp::{GoogleCloudStorage, GoogleCloudStorageBuilder};
use object_store::local::LocalFileSystem;
use scouter_error::StorageError;
use scouter_settings::{ObjectStorageSettings, StorageType};
use url::Url;

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
                StorageProvider::AWS(Arc::new(builder))
            }
            StorageType::Local => {
                let builder = LocalFileSystem::new_with_prefix(storage_settings.storage_root())
                    .map_err(|_| {
                        StorageError::ObjectStoreError(
                            "Failed to create local file system builder".to_string(),
                        )
                    })?;
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
            StorageProvider::AWS(store) => {
                let url = Url::parse(&storage_settings.storage_uri).map_err(|_| {
                    StorageError::ObjectStoreError("Failed to parse AWS S3 URI".to_string())
                })?;

                ctx.register_object_store(&url, store.clone());
            }
            StorageProvider::Local(store) => {
                let uri = if storage_settings.storage_uri.starts_with("file://") {
                    storage_settings.storage_uri.clone()
                } else {
                    format!("file://{}", storage_settings.storage_uri)
                };

                let url = Url::parse(&uri).map_err(|_| {
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
}

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
}
