use crate::producer::kafka::KafkaConfig;
use crate::producer::rabbitmq::RabbitMQConfig;
use pyo3::prelude::*;
use scouter_settings::HTTPConfig;

pub enum TransportConfig {
    RabbitMQ(RabbitMQConfig),
    Kafka(KafkaConfig),
    Http(HTTPConfig),
}

impl TransportConfig {
    pub fn from_py_config(config: &Bound<'_, PyAny>) -> PyResult<Self> {
        if let Ok(rabbitmq_config) = config.extract::<RabbitMQConfig>() {
            Ok(TransportConfig::RabbitMQ(rabbitmq_config))
        } else if let Ok(kafka_config) = config.extract::<KafkaConfig>() {
            Ok(TransportConfig::Kafka(kafka_config))
        } else if let Ok(http_config) = config.extract::<HTTPConfig>() {
            Ok(TransportConfig::Http(http_config))
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyTypeError, _>(
                "Invalid transport config type",
            ))
        }
    }
}
