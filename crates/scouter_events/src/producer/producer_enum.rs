#[cfg(any(feature = "kafka", feature = "kafka-vendored"))]
pub use crate::producer::kafka::KafkaProducer;

#[cfg(feature = "rabbitmq")]
pub use crate::producer::rabbitmq::RabbitMQProducer;

pub use crate::producer::http::HTTPProducer;
pub use crate::producer::kafka::KafkaConfig;
pub use crate::producer::rabbitmq::RabbitMQConfig;
use crate::queue::types::TransportConfig;

use crate::error::EventError;
use scouter_types::ServerRecords;
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

/// Underlying Enum used with feature queues
#[derive(Clone)]
pub struct RustScouterProducer {
    producer: ProducerEnum,
}

impl RustScouterProducer {
    pub async fn new(config: TransportConfig) -> Result<Self, EventError> {
        let producer = match config {
            TransportConfig::RabbitMQ(config) => {
                #[cfg(feature = "rabbitmq")]
                {
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
            }
            TransportConfig::Kafka(config) => {
                #[cfg(any(feature = "kafka", feature = "kafka-vendored"))]
                {
                    debug!("Creating Kafka producer");
                    ProducerEnum::Kafka(KafkaProducer::new(config)?)
                }
                #[cfg(not(any(feature = "kafka", feature = "kafka-vendored")))]
                {
                    return Err(PyScouterError::new_err(
                        "Kafka feature is not enabled".to_string(),
                    )
                    .into());
                }
            }
            TransportConfig::Http(config) => {
                let producer = HTTPProducer::new(config).await?;
                debug!("Creating HTTP producer");
                ProducerEnum::HTTP(producer)
            }
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
