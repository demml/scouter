use pyo3::prelude::*;
use pyo3::types::PyDict;
use rusty_logging::logger::LogLevel;
use scouter_error::{EventError, ScouterError};
use scouter_types::TransportTypes;
use std::collections::HashMap;
use std::env;
use std::str::FromStr;

#[pyclass(eq)]
#[derive(PartialEq, Clone, Debug)]
pub enum CompressionType {
    None,
    Gzip,
    Snappy,
    Lz4,
    Zstd,
}

impl FromStr for CompressionType {
    type Err = EventError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(CompressionType::None),
            "gzip" => Ok(CompressionType::Gzip),
            "snappy" => Ok(CompressionType::Snappy),
            "lz4" => Ok(CompressionType::Lz4),
            "zstd" => Ok(CompressionType::Zstd),
            _ => Err(EventError::InvalidCompressionTypeError),
        }
    }
}

// impl display
impl std::fmt::Display for CompressionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompressionType::None => write!(f, "none"),
            CompressionType::Gzip => write!(f, "gzip"),
            CompressionType::Snappy => write!(f, "snappy"),
            CompressionType::Lz4 => write!(f, "lz4"),
            CompressionType::Zstd => write!(f, "zstd"),
        }
    }
}

fn add_kafka_security(config: &mut HashMap<String, String>) -> Result<(), EventError> {
    if !config.contains_key("sasl.username") || !config.contains_key("sasl.password") {
        if let (Ok(sasl_username), Ok(sasl_password)) = (
            env::var("KAFKA_SASL_USERNAME"),
            env::var("KAFKA_SASL_PASSWORD"),
        ) {
            config.insert("sasl.username".to_string(), sasl_username);
            config.insert("sasl.password".to_string(), sasl_password);
            config.insert("security.protocol".to_string(), "SASL_SSL".to_string());
            config.insert("sasl.mechanisms".to_string(), "PLAIN".to_string());
        }
    }
    Ok(())
}

fn add_kafka_args(
    brokers: String,
    compression: CompressionType,
    message_timeout: u64,
    message_max_bytes: i32,
    config: &mut HashMap<String, String>,
) -> Result<(), EventError> {
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
    pub raise_on_error: bool,

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
    pub config_type: TransportTypes,
}

#[pymethods]
#[allow(clippy::too_many_arguments)]
impl KafkaConfig {
    #[new]
    #[pyo3(signature = (brokers=None, topic=None, compression_type=CompressionType::Gzip.to_string(), raise_on_error=false, message_timeout_ms=600000, message_max_bytes=2097164, log_level=LogLevel::Info, config=None, max_retries=3))]
    pub fn new(
        brokers: Option<String>,
        topic: Option<String>,
        compression_type: Option<String>,
        raise_on_error: Option<bool>,
        message_timeout_ms: Option<u64>,
        message_max_bytes: Option<i32>,
        log_level: Option<LogLevel>,
        config: Option<&Bound<'_, PyDict>>,
        max_retries: Option<i32>,
    ) -> Result<Self, ScouterError> {
        let brokers = brokers.unwrap_or_else(|| {
            env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_string())
        });
        let topic = topic.unwrap_or_else(|| {
            env::var("KAFKA_TOPIC").unwrap_or_else(|_| "scouter_monitoring".to_string())
        });
        let compression_type =
            CompressionType::from_str(&compression_type.unwrap_or("gzip".to_string()))?;
        let raise_on_error = raise_on_error.unwrap_or(false);
        let message_timeout_ms = message_timeout_ms.unwrap_or(600_000);
        let message_max_bytes = message_max_bytes.unwrap_or(2097164);

        let mut config = match config {
            Some(config) => config
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            None => HashMap::new(),
        };

        add_kafka_security(&mut config)?;
        add_kafka_args(
            brokers.clone(),
            compression_type.clone(),
            message_timeout_ms,
            message_max_bytes,
            &mut config,
        )?;

        let log_level = if let Some(level) = log_level {
            level
        } else {
            let env_var = env::var("LOG_LEVEL")
                .unwrap_or_else(|_| "info".to_string())
                .to_lowercase();
            match env_var.as_str() {
                "info" => LogLevel::Info,
                "debug" => LogLevel::Debug,
                "error" => LogLevel::Error,
                "warn" => LogLevel::Warn,
                "trace" => LogLevel::Trace,
                _ => LogLevel::Info,
            }
        };

        Ok(KafkaConfig {
            brokers,
            topic,
            compression_type,
            raise_on_error,
            message_timeout_ms,
            message_max_bytes,
            log_level,
            config,
            max_retries: max_retries.unwrap_or(3),
            config_type: TransportTypes::Kafka,
        })
    }
}
