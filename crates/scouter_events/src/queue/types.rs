use crate::producer::kafka::KafkaConfig;
use crate::producer::rabbitmq::RabbitMQConfig;
use scouter_settings::HTTPConfig;

pub enum TransportConfig {
    RabbitMQ(RabbitMQConfig),
    Kafka(KafkaConfig),
    Http(HTTPConfig),
}
