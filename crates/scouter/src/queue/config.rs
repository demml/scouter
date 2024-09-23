use crate::utils::types::json_to_pyobject;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::Value;
use std::collections::HashMap;
use tracing::info;
// create CompressionType enum

enum CompressionType {
    Gzip,
    Snappy,
    Lz4,
    Zstd,
    Inherit,
}

impl CompressionType {
    fn to_string(&self) -> String {
        match self {
            CompressionType::Gzip => "gzip".to_string(),
            CompressionType::Snappy => "snappy".to_string(),
            CompressionType::Lz4 => "lz4".to_string(),
            CompressionType::Zstd => "zstd".to_string(),
            CompressionType::Inherit => "inherit".to_string(),
        }
    }
}

fn check_compression_type(compression_type: &str) -> CompressionType {
    match compression_type {
        "gzip" => CompressionType::Gzip,
        "snappy" => CompressionType::Snappy,
        "lz4" => CompressionType::Lz4,
        "zstd" => CompressionType::Zstd,
        "inherit" => CompressionType::Inherit,
        _ => CompressionType::Gzip,
    }
}

enum ProducerType {
    Kafka,
}

impl ProducerType {
    fn to_string(&self) -> String {
        match self {
            ProducerType::Kafka => "Kafka".to_string(),
            ProducerType::Http => "http".to_string(),
        }
    }
}

#[pyclass]
pub struct KafkaConfig {
    #[pyo3(get, set)]
    brokers: String,

    #[pyo3(get, set)]
    topic: String,

    #[pyo3(get, set)]
    compression_type: String,

    #[pyo3(get, set)]
    raise_on_error: bool,

    #[pyo3(get, set)]
    message_timeout_ms: i32,

    #[pyo3(get, set)]
    message_max_bytes: i32,

    config: HashMap<String, String>,
}

impl KafkaConfig {
    fn finalize_config(mut config: HashMap<String, String>) -> HashMap<String, String> {
        // check if "sasl.username" and "sasl.password" are present in config

        if !config.contains_key("sasl.username") && !config.contains_key("sasl.password") {
            let sasl_username =
                std::env::var("KAFKA_SASL_USERNAME").unwrap_or_else(|_| "".to_string());
            let sasl_password =
                std::env::var("KAFKA_SASL_PASSWORD").unwrap_or_else(|_| "".to_string());

            if !sasl_username.is_empty() && !sasl_password.is_empty() {
                info!("Found SASL credentials in environment variables. Assigning security.protocol and sasl.mechanism");
                config.insert("sasl.username".to_string(), sasl_username);
                config.insert("sasl.password".to_string(), sasl_password);
                config.insert(
                    "security.protocol".to_string(),
                    std::env::var("KAFKA_SECURITY_PROTOCOL")
                        .unwrap_or_else(|_| "SASL_SSL".to_string()),
                );
                config.insert(
                    "sasl.mechanism".to_string(),
                    std::env::var("KAFKA_SASL_MECHANISM").unwrap_or_else(|_| "PLAIN".to_string()),
                );
            }
        }

        config
    }
}

#[pymethods]
impl KafkaConfig {
    #[new]
    fn new(
        brokers: Option<String>,
        topic: Option<String>,
        compression_type: Option<String>,
        raise_on_error: Option<bool>,
        message_timeout_ms: Option<i32>,
        message_max_bytes: Option<i32>,
        config: Option<HashMap<String, String>>,
    ) -> Self {
        // check env variables
        let brokers = brokers.unwrap_or_else(|| {
            std::env::var("KAFKA_BROKERS").unwrap_or_else(|_| "localhost:9092".to_string())
        });

        let topic = topic.unwrap_or_else(|| {
            std::env::var("KAFKA_TOPIC").unwrap_or_else(|_| "scouter_monitoring".to_string())
        });

        let compression_type = check_compression_type(&compression_type.unwrap_or_else(|| {
            std::env::var("KAFKA_COMPRESSION_TYPE").unwrap_or_else(|_| "none".to_string())
        }))
        .to_string();

        let raise_on_error = raise_on_error.unwrap_or_else(|| true);

        let message_timeout_ms = message_timeout_ms.unwrap_or_else(|| 600000);

        let message_max_bytes = message_max_bytes.unwrap_or_else(|| 2097164);

        let mut config = KafkaConfig::finalize_config(config.unwrap_or_else(|| HashMap::new()));

        config.insert("bootstrap.servers".to_string(), brokers.clone().to_string());
        config.insert("compression.type".to_string(), compression_type.clone());
        config.insert(
            "message.timeout.ms".to_string(),
            message_timeout_ms.to_string(),
        );
        config.insert(
            "message.max.bytes".to_string(),
            message_max_bytes.to_string(),
        );

        KafkaConfig {
            brokers,
            topic,
            compression_type,
            raise_on_error,
            message_timeout_ms,
            message_max_bytes,
            config,
        }
    }

    // property type
    #[getter]
    pub fn producer_type(&self) -> String {
        ProducerType::Kafka.to_string()
    }

    #[getter]
    pub fn config(&self, py: Python) -> PyResult<Py<PyDict>> {
        let json_str = serde_json::to_string(&self.config).unwrap();
        let json_value: Value = serde_json::from_str(&json_str).unwrap();

        // Create a new Python dictionary
        let dict = PyDict::new_bound(py);

        // Convert JSON to Python dict
        json_to_pyobject(py, &json_value, dict.as_gil_ref())?;

        // Return the Python dictionary
        Ok(dict.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kafka_config() {
        let kafka_config = KafkaConfig::new(
            Some("localhost:9092".to_string()),
            Some("scouter_monitoring".to_string()),
            Some("gzip".to_string()),
            Some(true),
            Some(600000),
            Some(2097164),
            Some(HashMap::new()),
        );

        assert_eq!(kafka_config.brokers, "localhost:9092");
        assert_eq!(kafka_config.topic, "scouter_monitoring");
        assert_eq!(kafka_config.compression_type, "gzip");
        assert_eq!(kafka_config.raise_on_error, true);
        assert_eq!(kafka_config.message_timeout_ms, 600000);
        assert_eq!(kafka_config.message_max_bytes, 2097164);
        assert_eq!(kafka_config.config.len(), 4);
    }
}
