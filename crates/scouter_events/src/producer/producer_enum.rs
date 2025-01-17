#[cfg(feature = "kafka")]
pub use crate::producer::kafka::{KafkaConfig, KafkaProducer};

#[cfg(feature = "rabbitmq")]
pub use crate::producer::rabbitmq::{RabbitMQConfig, RabbitMQProducer};

pub use crate::producer::http::{HTTPConfig, HTTPProducer};
use scouter_error::{PyScouterError, ScouterError};
use scouter_types::ServerRecords;
use std::sync::Arc;
use pyo3::prelude::*;

#[derive(Clone)]
pub enum ProducerEnum {
    HTTP(HTTPProducer),

    #[cfg(feature = "kafka")]
    Kafka(KafkaProducer),

    #[cfg(feature = "rabbitmq")]
    RabbitMQ(RabbitMQProducer),
}

impl ProducerEnum {
    pub async fn publish(&mut self, message: ServerRecords) -> Result<(), ScouterError> {
        match self {
            ProducerEnum::HTTP(producer) => producer.publish(message).await,
            #[cfg(feature = "kafka")]
            ProducerEnum::Kafka(producer) => producer.publish(message).await,
            #[cfg(feature = "rabbitmq")]
            ProducerEnum::RabbitMQ(producer) => producer.publish(message).await,
        }
    }

    pub async fn flush(&self) -> Result<(), ScouterError> {
        match self {
            ProducerEnum::HTTP(producer) => producer.flush(),
            #[cfg(feature = "kafka")]
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
            ProducerEnum::HTTP(producer)

        // check for kafka config
        } else if config.is_instance_of::<KafkaConfig>() {
            let config = config.extract::<KafkaConfig>()?;
            #[cfg(feature = "kafka")]
            {
                ProducerEnum::Kafka(KafkaProducer::new(config)?)
            }
            #[cfg(not(feature = "kafka"))]
            {
                return Err(
                    PyScouterError::new_err("Kafka feature is not enabled".to_string()).into(),
                );
            }

        // check for rabbitmq config
        } else if config.is_instance_of::<RabbitMQConfig>() {
            let config = config.extract::<RabbitMQConfig>()?;
            #[cfg(feature = "rabbitmq")]
            {
                let producer = rt.block_on(async { RabbitMQProducer::new(config).await })?;
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

        Ok(ScouterProducer {

            producer,
            rt,
        })
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
