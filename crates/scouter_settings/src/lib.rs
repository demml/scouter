use base64::prelude::*;
use scouter_types::StorageType;
use serde::Serialize;
use std::env;
use std::path::PathBuf;
use tracing::warn;

pub mod auth;
pub mod database;
pub mod events;
pub mod http;
pub mod polling;
pub mod storage;

pub use auth::AuthSettings;
pub use database::DatabaseSettings;
pub use events::{KafkaSettings, RabbitMQSettings};
pub use polling::PollingSettings;
pub use storage::ObjectStorageSettings;

fn generate_default_secret() -> String {
    // Creates a deterministic key for development purposes
    // Should be replaced with a proper secret in production
    let mut key = [0u8; 32];
    for (i, item) in key.iter_mut().enumerate() {
        // Different pattern than the JWT secret (reversed index)
        *item = (31 - i) as u8;
    }

    BASE64_STANDARD.encode(key)
}

#[derive(Debug, Clone, Serialize)]
pub struct ScouterServerConfig {
    pub polling_settings: PollingSettings,
    pub database_settings: DatabaseSettings,
    pub kafka_settings: Option<KafkaSettings>,
    pub rabbitmq_settings: Option<RabbitMQSettings>,
    pub auth_settings: AuthSettings,
    pub bootstrap_key: String,
    pub object_storage_settings: ObjectStorageSettings,
}

impl ScouterServerConfig {
    pub fn set_storage_uri(storage_uri: String) -> String {
        if storage_uri.starts_with("gs://")
            || storage_uri.starts_with("s3://")
            || storage_uri.starts_with("az://")
        {
            storage_uri
        } else {
            // For local storage, use a directory relative to where the process is running
            let path = if storage_uri.starts_with("./") || storage_uri.starts_with("../") {
                PathBuf::from(&storage_uri)
            } else {
                // If it's not a relative path, make it one explicitly relative to current dir
                PathBuf::from("./").join(&storage_uri)
            };

            // Create directory if it doesn't exist
            if !path.exists() {
                std::fs::create_dir_all(&path).unwrap();
            }

            // Return path as string (not canonicalized)
            path.to_str().unwrap().to_string()
        }
    }

    fn get_storage_type(storage_uri: &str) -> StorageType {
        let storage_uri_lower = storage_uri.to_lowercase();
        if storage_uri_lower.starts_with("gs://") {
            StorageType::Google
        } else if storage_uri_lower.starts_with("s3://") {
            StorageType::Aws
        } else if storage_uri_lower.starts_with("az://") {
            StorageType::Azure
        } else {
            StorageType::Local
        }
    }
}

impl Default for ScouterServerConfig {
    fn default() -> Self {
        let polling = PollingSettings::default();
        let database = DatabaseSettings::default();
        let kafka = if std::env::var("KAFKA_BROKERS").is_ok() {
            Some(KafkaSettings::default())
        } else {
            None
        };

        let rabbitmq = if std::env::var("RABBITMQ_ADDR").is_ok() {
            Some(RabbitMQSettings::default())
        } else {
            None
        };

        let auth_settings = AuthSettings {
            jwt_secret: env::var("SCOUTER_ENCRYPT_SECRET").unwrap_or_else(|_| {
                warn!(
                    "Using default secret for encryption 
                        This is not recommended for production use."
                );
                generate_default_secret()
            }),
            refresh_secret: env::var("SCOUTER_REFRESH_SECRET").unwrap_or_else(|_| {
                warn!(
                    "Using default secret for refreshing. 
                        This is not recommended for production use."
                );

                generate_default_secret()
            }),
        };

        let bootstrap_key =
            env::var("SCOUTER_BOOTSTRAP_KEY").unwrap_or_else(|_| generate_default_secret());

        Self {
            polling_settings: polling,
            database_settings: database,
            kafka_settings: kafka,
            rabbitmq_settings: rabbitmq,
            auth_settings,
            bootstrap_key,
            object_storage_settings: ObjectStorageSettings::default(),
        }
    }
}

impl ScouterServerConfig {
    pub fn kafka_enabled(&self) -> bool {
        self.kafka_settings.is_some()
    }

    pub fn rabbitmq_enabled(&self) -> bool {
        self.rabbitmq_settings.is_some()
    }
}
