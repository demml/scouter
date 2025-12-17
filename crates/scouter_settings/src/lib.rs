use base64::prelude::*;
use scouter_types::StorageType;
use std::env;
use tracing::warn;

pub mod auth;
pub mod database;
pub mod events;
pub mod grpc;
pub mod http;
pub mod llm;
pub mod polling;
pub mod storage;

use crate::events::HttpConsumerSettings;
pub use auth::AuthSettings;
pub use database::DatabaseSettings;
pub use events::{KafkaSettings, RabbitMQSettings, RedisSettings};
pub use http::HttpConfig;
pub use llm::LLMSettings;
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

#[derive(Debug, Clone)]
pub struct ScouterServerConfig {
    pub polling_settings: PollingSettings,
    pub database_settings: DatabaseSettings,
    pub kafka_settings: Option<KafkaSettings>,
    pub rabbitmq_settings: Option<RabbitMQSettings>,
    pub redis_settings: Option<RedisSettings>,
    pub http_consumer_settings: HttpConsumerSettings,
    pub llm_settings: LLMSettings,
    pub auth_settings: AuthSettings,
    pub bootstrap_key: String,
    pub storage_settings: ObjectStorageSettings,
}

impl ScouterServerConfig {
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

impl ScouterServerConfig {
    pub async fn new() -> Self {
        let polling = PollingSettings::default();
        let database = DatabaseSettings::default();
        let kafka = if env::var("KAFKA_BROKERS").is_ok() {
            Some(KafkaSettings::default())
        } else {
            None
        };

        let rabbitmq = if env::var("RABBITMQ_ADDR").is_ok() {
            Some(RabbitMQSettings::default())
        } else {
            None
        };

        let redis = if std::env::var("REDIS_ADDR").is_ok() {
            Some(RedisSettings::default())
        } else {
            None
        };
        let http_consumer_settings = HttpConsumerSettings::default();

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

        let llm_settings = LLMSettings::new().await;

        Self {
            polling_settings: polling,
            database_settings: database,
            kafka_settings: kafka,
            rabbitmq_settings: rabbitmq,
            redis_settings: redis,
            auth_settings,
            bootstrap_key,
            llm_settings,
            http_consumer_settings,
            storage_settings: ObjectStorageSettings::default(),
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

    pub fn redis_enabled(&self) -> bool {
        self.redis_settings.is_some()
    }

    pub fn llm_enabled(&self) -> bool {
        self.llm_settings.is_configured()
    }
}
