use crate::error::PyEventError;
use crate::producer::kafka::KafkaConfig;
use crate::producer::mock::MockConfig;
use crate::producer::rabbitmq::RabbitMQConfig;
use crate::producer::redis::RedisConfig;
use pyo3::prelude::*;
use pyo3::IntoPyObjectExt;
use scouter_settings::grpc::GrpcConfig;
use scouter_settings::HttpConfig;
use scouter_types::TransportType;
use std::collections::HashMap;
use tracing::error;
use tracing::instrument;

/// Individual queue settings
/// this is currently a hacky way to mutate sample ratios for GenAI queues when
/// using tracing. The main problem to be solved here is communicating with the background
/// task that is processing the queue to update the sample ratio from python.
pub(crate) struct QueueSettings {
    pub id: String,
    pub sample_ratio: f64,
}

impl QueueSettings {
    pub fn new(id: String, sample_ratio: f64) -> Self {
        QueueSettings { id, sample_ratio }
    }

    /// updates all sample ratios in the settings
    /// this is only used by the GenAI queues currently
    pub fn update_sample_ratio(&mut self, sample_ratio: f64) {
        self.sample_ratio = sample_ratio;
    }
}

#[derive(Clone, Debug)]
pub enum TransportConfig {
    RabbitMQ(RabbitMQConfig),
    Kafka(KafkaConfig),
    Http(HttpConfig),
    Redis(RedisConfig),
    Mock(MockConfig),
    Grpc(GrpcConfig),
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
    #[instrument(skip_all)]
    pub fn from_py_config(config: &Bound<'_, PyAny>) -> PyResult<Self> {
        let transport_type = config.getattr("transport_type")?;

        let extracted_type = transport_type.extract::<TransportType>().map_err(|e| {
            error!("Failed to extract transport type: {}", e);
            e
        })?;

        match extracted_type {
            TransportType::RabbitMQ => {
                let rabbitmq_config = config.extract::<RabbitMQConfig>()?;
                Ok(TransportConfig::RabbitMQ(rabbitmq_config))
            }
            TransportType::Kafka => {
                let kafka_config = config.extract::<KafkaConfig>()?;
                Ok(TransportConfig::Kafka(kafka_config))
            }
            TransportType::Http => {
                let http_config = config.extract::<HttpConfig>()?;
                Ok(TransportConfig::Http(http_config))
            }
            TransportType::Redis => {
                let redis_config = config.extract::<RedisConfig>()?;
                Ok(TransportConfig::Redis(redis_config))
            }
            TransportType::Grpc => {
                let grpc_config = config.extract::<GrpcConfig>()?;
                Ok(TransportConfig::Grpc(grpc_config))
            }
            TransportType::Mock => {
                let mock_config = config.extract::<MockConfig>()?;
                Ok(TransportConfig::Mock(mock_config))
            }
        }
    }

    /// helper method to convert the TransportConfig to a python object
    pub fn to_py<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyAny>, PyEventError> {
        let transport = match self {
            TransportConfig::RabbitMQ(config) => config.clone().into_bound_py_any(py),
            TransportConfig::Kafka(config) => config.clone().into_bound_py_any(py),
            TransportConfig::Http(config) => config.clone().into_bound_py_any(py),
            TransportConfig::Redis(config) => config.clone().into_bound_py_any(py),
            TransportConfig::Mock(config) => config.clone().into_bound_py_any(py),
            TransportConfig::Grpc(config) => config.clone().into_bound_py_any(py),
        };

        match transport {
            Ok(t) => Ok(t),
            Err(e) => {
                error!("Failed to convert TransportConfig to Python object: {}", e);
                Err(PyEventError::ConvertToPyError(e))
            }
        }
    }
}
