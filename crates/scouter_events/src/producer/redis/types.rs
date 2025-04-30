use pyo3::prelude::*;
use scouter_types::TransportType;

#[pyclass]
#[derive(Debug, Clone)]
pub struct RedisConfig {
    #[pyo3(get)]
    pub channel: String,

    #[pyo3(get)]
    pub address: String,

    #[pyo3(get)]
    pub transport_type: TransportType,
}

#[pymethods]
impl RedisConfig {
    #[new]
    #[pyo3(signature = (address=None, channel=None))]
    pub fn new(address: Option<String>, channel: Option<String>) -> Self {
        let address = address.unwrap_or_else(|| {
            std::env::var("REDIS_ADDR").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string())
        });

        let channel = channel.unwrap_or_else(|| {
            std::env::var("REDIS_CHANNEL").unwrap_or_else(|_| "scouter_monitoring".to_string())
        });

        Self {
            address,
            channel,
            transport_type: TransportType::Redis,
        }
    }
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self::new(None, None)
    }
}
