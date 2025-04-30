use pyo3::prelude::*;
use serde::Serialize;

#[pyclass]
#[derive(Debug, Clone, Serialize)]
pub struct RedisConfig {
    pub channel: String,
    pub address: String,
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

        Self { address, channel }
    }
}
