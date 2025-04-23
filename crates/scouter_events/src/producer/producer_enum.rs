#[cfg(any(feature = "kafka", feature = "kafka-vendored"))]
pub use crate::producer::kafka::KafkaProducer;

#[cfg(feature = "rabbitmq")]
pub use crate::producer::rabbitmq::RabbitMQProducer;

pub use crate::producer::http::HTTPProducer;
pub use crate::producer::kafka::KafkaConfig;
pub use crate::producer::rabbitmq::RabbitMQConfig;

use pyo3::prelude::*;
use scouter_error::{EventError, PyScouterError, ScouterError};
use scouter_settings::HTTPConfig;
use scouter_types::ServerRecords;
use std::sync::Arc;
use tracing::debug;

#[derive(Clone)]
pub enum ProducerEnum {
    HTTP(HTTPProducer),

    #[cfg(any(feature = "kafka", feature = "kafka-vendored"))]
    Kafka(KafkaProducer),

    #[cfg(feature = "rabbitmq")]
    RabbitMQ(RabbitMQProducer),
}

impl ProducerEnum {
    pub async fn publish(&mut self, message: ServerRecords) -> Result<(), EventError> {
        match self {
            ProducerEnum::HTTP(producer) => producer.publish(message).await,
            #[cfg(any(feature = "kafka", feature = "kafka-vendored"))]
            ProducerEnum::Kafka(producer) => producer.publish(message).await,
            #[cfg(feature = "rabbitmq")]
            ProducerEnum::RabbitMQ(producer) => producer.publish(message).await,
        }
    }

    pub async fn flush(&self) -> Result<(), EventError> {
        match self {
            ProducerEnum::HTTP(producer) => producer.flush().await,
            #[cfg(any(feature = "kafka", feature = "kafka-vendored"))]
            ProducerEnum::Kafka(producer) => producer.flush(),
            #[cfg(feature = "rabbitmq")]
            ProducerEnum::RabbitMQ(producer) => producer.flush().await,
        }
    }
}

#[pyclass]
#[derive(Clone)]
pub struct ScouterProducer {
    producer: ProducerEnum,
    pub rt: Arc<tokio::runtime::Runtime>,
}

#[pymethods]
impl ScouterProducer {
    #[new]
    #[pyo3(signature = (config))]
    pub fn new(config: &Bound<'_, PyAny>) -> Result<Self, ScouterError> {
        // create tokio runtime for handling async funcs
        let rt = Arc::new(tokio::runtime::Runtime::new().unwrap());

        // check for http config
        let producer = if config.is_instance_of::<HTTPConfig>() {
            let config = config.extract::<HTTPConfig>()?;
            let producer = rt.block_on(async { HTTPProducer::new(config).await })?;
            debug!("Creating HTTP producer");
            ProducerEnum::HTTP(producer)

        // check for kafka config
        } else if config.is_instance_of::<KafkaConfig>() {
            #[cfg(any(feature = "kafka", feature = "kafka-vendored"))]
            {
                let config = config.extract::<KafkaConfig>()?;
                debug!("Creating Kafka producer");
                ProducerEnum::Kafka(KafkaProducer::new(config)?)
            }
            #[cfg(not(any(feature = "kafka", feature = "kafka-vendored")))]
            {
                return Err(
                    PyScouterError::new_err("Kafka feature is not enabled".to_string()).into(),
                );
            }

        // check for rabbitmq config
        } else if config.is_instance_of::<RabbitMQConfig>() {
            #[cfg(feature = "rabbitmq")]
            {
                let config = config.extract::<RabbitMQConfig>()?;
                let producer = rt.block_on(async { RabbitMQProducer::new(config).await })?;
                debug!("Creating RabbitMQ producer");
                ProducerEnum::RabbitMQ(producer)
            }
            #[cfg(not(feature = "rabbitmq"))]
            {
                return Err(
                    ScouterError::Error("RabbitMQ feature is not enabled".to_string()).into(),
                );
            }

        // fail
        } else {
            return Err(PyScouterError::new_err("Invalid config".to_string()).into());
        };

        Ok(ScouterProducer { producer, rt })
    }

    pub fn publish(&mut self, message: ServerRecords) -> Result<(), ScouterError> {
        self.rt
            .block_on(async { self.producer.publish(message).await })?;
        Ok(())
    }

    pub fn flush(&self) -> Result<(), ScouterError> {
        self.rt.block_on(async { self.producer.flush().await })?;
        Ok(())
    }
}

/// Underlying Enum used with feature queues
#[derive(Clone)]
pub struct RustScouterProducer {
    producer: ProducerEnum,
}

impl RustScouterProducer {
    pub async fn new(config: &Bound<'_, PyAny>) -> Result<Self, ScouterError> {
        // check for http config
        let producer = if config.is_instance_of::<HTTPConfig>() {
            let config = config.extract::<HTTPConfig>()?;
            let producer = HTTPProducer::new(config).await?;
            debug!("Creating HTTP producer");
            ProducerEnum::HTTP(producer)

        // check for kafka config
        } else if config.is_instance_of::<KafkaConfig>() {
            #[cfg(any(feature = "kafka", feature = "kafka-vendored"))]
            {
                let config = config.extract::<KafkaConfig>()?;
                debug!("Creating Kafka producer");
                ProducerEnum::Kafka(KafkaProducer::new(config)?)
            }
            #[cfg(not(any(feature = "kafka", feature = "kafka-vendored")))]
            {
                return Err(
                    PyScouterError::new_err("Kafka feature is not enabled".to_string()).into(),
                );
            }

        // check for rabbitmq config
        } else if config.is_instance_of::<RabbitMQConfig>() {
            #[cfg(feature = "rabbitmq")]
            {
                let config = config.extract::<RabbitMQConfig>()?;
                let producer = RabbitMQProducer::new(config).await?;
                debug!("Creating RabbitMQ producer");
                ProducerEnum::RabbitMQ(producer)
            }
            #[cfg(not(feature = "rabbitmq"))]
            {
                return Err(
                    ScouterError::Error("RabbitMQ feature is not enabled".to_string()).into(),
                );
            }

        // fail
        } else {
            return Err(PyScouterError::new_err("Invalid config".to_string()).into());
        };

        Ok(RustScouterProducer { producer })
    }

    pub async fn publish(&mut self, message: ServerRecords) -> Result<(), EventError> {
        debug!("message length: {}", message.len());
        self.producer.publish(message).await
    }

    pub async fn flush(&self) -> Result<(), EventError> {
        self.producer.flush().await
    }
}
