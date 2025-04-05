use object_store::aws::{AmazonS3, AmazonS3Builder};
use object_store::azure::{MicrosoftAzure, MicrosoftAzureBuilder};
use object_store::gcp::{GoogleCloudStorage, GoogleCloudStorageBuilder};
use object_store::local::LocalFileSystem;
use scouter_error::StorageError;
use scouter_settings::{ObjectStorageSettings, StorageType};

pub enum ObjectStore {
    Google(GoogleCloudStorage),
    AWS(AmazonS3),
    Local(LocalFileSystem),
    Azure(MicrosoftAzure),
}

impl ObjectStore {
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
                ObjectStore::Google(builder)
            }
            StorageType::Aws => {
                let builder = AmazonS3Builder::from_env()
                    .with_bucket_name(storage_settings.storage_root())
                    .build()
                    .map_err(|_| {
                        StorageError::ObjectStoreError(
                            "Failed to create AWS S3 builder".to_string(),
                        )
                    })?;
                ObjectStore::AWS(builder)
            }
            StorageType::Local => {
                let builder = LocalFileSystem::new_with_prefix(storage_settings.storage_root())
                    .map_err(|_| {
                        StorageError::ObjectStoreError(
                            "Failed to create local file system builder".to_string(),
                        )
                    })?;
                ObjectStore::Local(builder)
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
                ObjectStore::Azure(builder)
            }
        };

        Ok(store)
    }
}
