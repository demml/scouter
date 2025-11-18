use crate::error::PyEventError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use rusty_logging::logger::LogLevel;
use scouter_types::{CompressionType, TransportType};
use std::collections::HashMap;
use std::env;
use std::str::FromStr;

fn add_kafka_args(
    brokers: String,
    compression: CompressionType,
    message_timeout: u64,
    message_max_bytes: i32,
    config: &mut HashMap<String, String>,
) -> Result<(), PyEventError> {
    config.insert("bootstrap.servers".to_string(), brokers);
    config.insert("compression.type".to_string(), compression.to_string());
    config.insert(
        "message.timeout.ms".to_string(),
        message_timeout.to_string(),
    );
    config.insert(
        "message.max.bytes".to_string(),
        message_max_bytes.to_string(),
    );
    Ok(())
}

#[pyclass]
#[derive(Clone, Debug)]
pub struct KafkaConfig {
    #[pyo3(get, set)]
    pub brokers: String,

    #[pyo3(get, set)]
    pub topic: String,

    #[pyo3(get, set)]
    pub compression_type: CompressionType,

    #[pyo3(get, set)]
    pub message_timeout_ms: u64,

    #[pyo3(get, set)]
    pub message_max_bytes: i32,

    #[pyo3(get, set)]
    pub log_level: LogLevel,

    #[pyo3(get, set)]
    pub config: HashMap<String, String>,

    #[pyo3(get, set)]
    pub max_retries: i32,

    #[pyo3(get)]
    pub transport_type: TransportType,
}

#[pymethods]
#[allow(clippy::too_many_arguments)]
impl KafkaConfig {
    #[new]
    #[pyo3(signature = (username=None, password=None,brokers=None, topic=None, compression_type=CompressionType::Gzip.to_string(), message_timeout_ms=600000, message_max_bytes=2097164, log_level=LogLevel::Info, config=None, max_retries=3))]
    pub fn new(
        username: Option<String>,
        password: Option<String>,
        brokers: Option<String>,
        topic: Option<String>,
        compression_type: Option<String>,
        message_timeout_ms: Option<u64>,
        message_max_bytes: Option<i32>,
        log_level: Option<LogLevel>,
        config: Option<&Bound<'_, PyDict>>,
        max_retries: Option<i32>,
    ) -> Result<Self, PyEventError> {
        let username = username.or_else(|| {
            std::env::var("KAFKA_USERNAME")
                .ok()
                .or_else(|| std::env::var("KAFKA_KEY").ok())
        });

        let password = password.or_else(|| {
            std::env::var("KAFKA_PASSWORD")
                .ok()
                .or_else(|| std::env::var("KAFKA_SECRET").ok())
        });

        let brokers = brokers.unwrap_or_else(|| {
            env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_string())
        });
        let topic = topic.unwrap_or_else(|| {
            env::var("SCOUTER_KAFKA_TOPIC")
                .or_else(|_| env::var("KAFKA_TOPIC"))
                .unwrap_or_else(|_| "scouter_monitoring".to_string())
        });
        let compression_type =
            CompressionType::from_str(&compression_type.unwrap_or_else(|| "gzip".to_string()))?;
        let message_timeout_ms = message_timeout_ms.unwrap_or(600_000);
        let message_max_bytes = message_max_bytes.unwrap_or(2097164);

        let mut config = match config {
            Some(config) => config
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            None => HashMap::new(),
        };

        // add username and password if provided and not already in config
        Self::add_sasl_credentials(&mut config, username, password);

        add_kafka_args(
            brokers.clone(),
            compression_type.clone(),
            message_timeout_ms,
            message_max_bytes,
            &mut config,
        )?;

        let log_level = Self::resolve_log_level(log_level);

        Ok(KafkaConfig {
            brokers,
            topic,
            compression_type,
            message_timeout_ms,
            message_max_bytes,
            log_level,
            config,
            max_retries: max_retries.unwrap_or(3),
            transport_type: TransportType::Kafka,
        })
    }
}

impl KafkaConfig {
    fn add_sasl_credentials(
        config: &mut HashMap<String, String>,
        username: Option<String>,
        password: Option<String>,
    ) {
        // Only add credentials if both are provided and neither key exists in config
        if !config.contains_key("sasl.username") && !config.contains_key("sasl.password") {
            if let (Some(username), Some(password)) = (username, password) {
                config.insert("sasl.username".to_string(), username);
                config.insert("sasl.password".to_string(), password);

                // If security protocol and sasl mechanism are not set, use defaults
                if !config.contains_key("security.protocol") {
                    let security_protocol = std::env::var("KAFKA_SECURITY_PROTOCOL")
                        .unwrap_or_else(|_| "SASL_SSL".to_string());
                    config.insert("security.protocol".to_string(), security_protocol);
                }

                if !config.contains_key("sasl.mechanism") {
                    let sasl_mechanism = std::env::var("KAFKA_SASL_MECHANISM")
                        .unwrap_or_else(|_| "PLAIN".to_string());
                    config.insert("sasl.mechanism".to_string(), sasl_mechanism);
                }
            }
        }
    }

    /// Resolve log level from parameter or environment variable
    fn resolve_log_level(log_level: Option<LogLevel>) -> LogLevel {
        log_level.unwrap_or_else(|| {
            let env_var = env::var("LOG_LEVEL")
                .unwrap_or_else(|_| "info".to_string())
                .to_lowercase();
            match env_var.as_str() {
                "debug" => LogLevel::Debug,
                "error" => LogLevel::Error,
                "warn" => LogLevel::Warn,
                "trace" => LogLevel::Trace,
                _ => LogLevel::Info,
            }
        })
    }
}
