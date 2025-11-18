#[cfg(any(feature = "kafka", feature = "kafka-vendored"))]
pub use crate::producer::kafka::KafkaProducer;

#[cfg(feature = "rabbitmq")]
pub use crate::producer::rabbitmq::RabbitMQProducer;

#[cfg(feature = "redis_events")]
use crate::producer::redis::RedisProducer;

pub use crate::producer::http::HTTPProducer;
pub use crate::producer::kafka::KafkaConfig;
pub use crate::producer::mock::{MockConfig, MockProducer};
pub use crate::producer::rabbitmq::RabbitMQConfig;
use crate::queue::types::TransportConfig;

use crate::error::EventError;
use scouter_types::MessageRecord;
use tracing::debug;

#[derive(Clone)]
pub enum ProducerEnum {
    HTTP(HTTPProducer),

    Mock(MockProducer),

    #[cfg(any(feature = "kafka", feature = "kafka-vendored"))]
    Kafka(KafkaProducer),

    #[cfg(feature = "rabbitmq")]
    RabbitMQ(RabbitMQProducer),

    #[cfg(feature = "redis_events")]
    Redis(RedisProducer),
}

impl ProducerEnum {
    pub async fn publish(&self, message: MessageRecord) -> Result<(), EventError> {
        match self {
            // this has mut
            ProducerEnum::HTTP(producer) => producer.publish(message).await,
            ProducerEnum::Mock(producer) => producer.publish(message).await,
            #[cfg(any(feature = "kafka", feature = "kafka-vendored"))]
            ProducerEnum::Kafka(producer) => producer.publish(message).await,
            #[cfg(feature = "rabbitmq")]
            ProducerEnum::RabbitMQ(producer) => producer.publish(message).await,
            #[cfg(feature = "redis_events")]
            ProducerEnum::Redis(producer) => producer.publish(message).await,
        }
    }

    pub async fn flush(&self) -> Result<(), EventError> {
        match self {
            ProducerEnum::HTTP(producer) => producer.flush().await,
            ProducerEnum::Mock(producer) => producer.flush().await,
            #[cfg(any(feature = "kafka", feature = "kafka-vendored"))]
            ProducerEnum::Kafka(producer) => producer.flush(),
            #[cfg(feature = "rabbitmq")]
            ProducerEnum::RabbitMQ(producer) => producer.flush().await,
            #[cfg(feature = "redis_events")]
            ProducerEnum::Redis(producer) => producer.flush().await,
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
            TransportConfig::RabbitMQ(_config) => {
                #[cfg(feature = "rabbitmq")]
                {
                    let producer = RabbitMQProducer::new(_config).await?;
                    debug!("Creating RabbitMQ producer");
                    ProducerEnum::RabbitMQ(producer)
                }
                #[cfg(not(feature = "rabbitmq"))]
                {
                    return Err(EventError::RabbitMQFeatureNotEnabledError);
                }
            }
            TransportConfig::Kafka(_config) => {
                #[cfg(any(feature = "kafka", feature = "kafka-vendored"))]
                {
                    debug!("Creating Kafka producer");
                    ProducerEnum::Kafka(KafkaProducer::new(_config)?)
                }
                #[cfg(not(any(feature = "kafka", feature = "kafka-vendored")))]
                {
                    return Err(EventError::KafkaFeatureNotEnabledError);
                }
            }
            TransportConfig::Redis(_config) => {
                #[cfg(feature = "redis_events")]
                {
                    let producer = RedisProducer::new(_config).await?;
                    debug!("Creating Redis producer");
                    ProducerEnum::Redis(producer)
                }
                #[cfg(not(feature = "redis_events"))]
                {
                    return Err(EventError::RedisFeatureNotEnabledError);
                }
            }
            TransportConfig::Http(config) => {
                debug!("Creating HTTP producer");
                let producer = HTTPProducer::new(config).await?;
                ProducerEnum::HTTP(producer)
            }
            TransportConfig::Mock(config) => {
                let producer = MockProducer::new(config).await?;
                debug!("Creating Mock producer");
                ProducerEnum::Mock(producer)
            }
        };

        Ok(RustScouterProducer { producer })
    }

    pub async fn publish(&self, message: MessageRecord) -> Result<(), EventError> {
        debug!("message length: {}", message.len());
        self.producer.publish(message).await
    }

    pub async fn flush(&self) -> Result<(), EventError> {
        self.producer.flush().await
    }
}
