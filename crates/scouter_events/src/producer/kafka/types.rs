use pyo3::prelude::*;
use pyo3::types::PyDict;
use scouter_error::ScouterError;
use std::env;

#[pyclass(eq)]
#[derive(PartialEq, Clone)]
pub enum CompressionType {
    None,
    Gzip,
    Snappy,
    Lz4,
    Zstd,
}


impl CompressionType {
    pub fn from_str(compression_type: &str) -> Result<CompressionType, ScouterError> {
        match compression_type {
            "none" => Ok(CompressionType::None),
            "gzip" => Ok(CompressionType::Gzip),
            "snappy" => Ok(CompressionType::Snappy),
            "lz4" => Ok(CompressionType::Lz4),
            "zstd" => Ok(CompressionType::Zstd),
            _ => Err(ScouterError::Error("Invalid compression type".to_string())),
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

fn add_kafka_security<'py>(config: &Bound<'py, PyDict>) -> Result<Bound<'py, PyDict>, ScouterError> {
    if !config.contains("sasl.username")? || !config.contains("sasl.password")? {
        if let (Ok(sasl_username), Ok(sasl_password)) = (env::var("KAFKA_SASL_USERNAME"), env::var("KAFKA_SASL_PASSWORD")) {
            config.set_item("sasl.username", sasl_username).map_err(|e| ScouterError::Error(e.to_string()))?;
            config.set_item("sasl.password", sasl_password).map_err(|e| ScouterError::Error(e.to_string()))?;
            config.set_item("security.protocol", "SASL_SSL").map_err(|e| ScouterError::Error(e.to_string()))?;
            config.set_item("sasl.mechanisms", "PLAIN").map_err(|e| ScouterError::Error(e.to_string()))?;
        }
    }
    Ok(config.clone())
}

fn add_kafka_args<'py>(brokers:String, compression:CompressionType, message_timeout: i32, message_max_bytes:i32, config: &Bound<'py, PyDict>) -> Result<Bound<'py, PyDict>, ScouterError> {
    config.set_item("bootstrap.servers", brokers).map_err(|e| ScouterError::Error(e.to_string()))?;
    config.set_item("compression.type", compression.to_string()).map_err(|e| ScouterError::Error(e.to_string()))?;
    config.set_item("message.timeout.ms", message_timeout).map_err(|e| ScouterError::Error(e.to_string()))?;
    config.set_item("message.max.bytes", message_max_bytes).map_err(|e| ScouterError::Error(e.to_string()))?;
    Ok(config.clone())
}

#[pyclass]
pub struct KafkaConfig {
    pub brokers: String,
    pub topic: String,
    pub compression_type: CompressionType,
    pub raise_on_error: bool,
    pub message_timeout_ms: i32,
    pub message_max_bytes: i32,
    pub config: Py<PyDict>,

}

#[pymethods]
impl KafkaConfig {
    #[new]
    #[pyo3(signature = (brokers=None, topic=None, compression_type=CompressionType::Gzip.to_string(), raise_on_error=false, message_timeout_ms=600000, message_max_bytes=2097164, config=None))]
    pub fn new(
        py: Python,
        brokers: Option<String>,
        topic: Option<String>,
        compression_type: Option<String>,
        raise_on_error: Option<bool>,
        message_timeout_ms: Option<i32>,
        message_max_bytes:  Option<i32>,
        config: Option<&Bound<'_, PyDict>>,
    ) -> Result<Self, ScouterError> {
        let brokers = brokers.unwrap_or_else(|| env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_string()));
        let topic = topic.unwrap_or_else(|| env::var("KAFKA_TOPIC").unwrap_or_else(|_| "scouter_monitoring".to_string()));
        let compression_type = CompressionType::from_str(&compression_type.unwrap_or("gzip".to_string()))?;
        let raise_on_error = raise_on_error.unwrap_or(false);
        let message_timeout_ms = message_timeout_ms.unwrap_or(600_000);
        let message_max_bytes = message_max_bytes.unwrap_or(2097164);
        
        // if config is None, create a new PyDict, else unwrap the config
        let config = match config {
            Some(config) => config.clone(),
            None => PyDict::new(py),
        };

        let config = add_kafka_security(&config)?;
        let config = add_kafka_args(brokers.clone(), compression_type.clone(), message_timeout_ms, message_max_bytes, &config)?;
        

        Ok(KafkaConfig {
            brokers,
            topic,
            compression_type,
            raise_on_error,
            message_timeout_ms,
            message_max_bytes,
            config: config.unbind(),
        })
    }
}