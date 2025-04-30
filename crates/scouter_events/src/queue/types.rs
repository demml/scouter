use crate::producer::kafka::KafkaConfig;
use crate::producer::rabbitmq::RabbitMQConfig;
use crate::producer::redis::RedisConfig;
use pyo3::prelude::*;
use pyo3::IntoPyObjectExt;
use scouter_settings::HTTPConfig;
use scouter_types::TransportTypes;

#[derive(Clone, Debug)]
pub enum TransportConfig {
    RabbitMQ(RabbitMQConfig),
    Kafka(KafkaConfig),
    Http(HTTPConfig),
    Redis(RedisConfig),
}

impl TransportConfig {
    /// Create a TransportConfig from a python config object.
    /// Function will extract the transport type and then extract the corresponding config
    /// before returning the TransportConfig.
    ///
    /// # Arguments
    /// * `config` - Python config object
    ///
    /// # Returns
    /// * `TransportConfig` - TransportConfig object
    pub fn from_py_config(config: &Bound<'_, PyAny>) -> PyResult<Self> {
        let transport_type = config
            .getattr("transport_type")?
            .extract::<TransportTypes>()?;

        match transport_type {
            TransportTypes::RabbitMQ => {
                let rabbitmq_config = config.extract::<RabbitMQConfig>()?;
                Ok(TransportConfig::RabbitMQ(rabbitmq_config))
            }
            TransportTypes::Kafka => {
                let kafka_config = config.extract::<KafkaConfig>()?;
                Ok(TransportConfig::Kafka(kafka_config))
            }
            TransportTypes::Http => {
                let http_config = config.extract::<HTTPConfig>()?;
                Ok(TransportConfig::Http(http_config))
            }
            TransportTypes::Redis => {
                let redis_config = config.extract::<RedisConfig>()?;
                Ok(TransportConfig::Redis(redis_config))
            }
        }
    }

    /// helper method to convert the TransportConfig to a python object
    pub fn to_py<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match self {
            TransportConfig::RabbitMQ(config) => config.clone().into_bound_py_any(py),
            TransportConfig::Kafka(config) => config.clone().into_bound_py_any(py),
            TransportConfig::Http(config) => config.clone().into_bound_py_any(py),
            TransportConfig::Redis(config) => config.clone().into_bound_py_any(py),
        }
    }
}
