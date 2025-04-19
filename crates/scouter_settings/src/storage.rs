use crate::ScouterServerConfig;
use scouter_types::StorageType;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct ObjectStorageSettings {
    pub storage_uri: String,
    pub storage_type: StorageType,
    pub region: String, // this is aws specific
}

impl Default for ObjectStorageSettings {
    fn default() -> Self {
        let storage_uri = std::env::var("SCOUTER_STORAGE_URI")
            .unwrap_or_else(|_| "./scouter_storage".to_string());

        let storage_type = ScouterServerConfig::get_storage_type(&storage_uri);

        // need to set this for aws objectstore
        let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());

        Self {
            storage_uri,
            storage_type,
            region,
        }
    }
}

impl ObjectStorageSettings {
    pub fn storage_root(&self) -> String {
        match self.storage_type {
            StorageType::Google | StorageType::Aws | StorageType::Azure => {
                if let Some(stripped) = self.storage_uri.strip_prefix("gs://") {
                    stripped.to_string()
                } else if let Some(stripped) = self.storage_uri.strip_prefix("s3://") {
                    stripped.to_string()
                } else if let Some(stripped) = self.storage_uri.strip_prefix("az://") {
                    stripped.to_string()
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
