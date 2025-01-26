#[cfg(feature = "rabbitmq")]
pub mod rabbitmq_producer {
    use crate::producer::rabbitmq::types::RabbitMQConfig;
    use lapin::{
        options::{BasicPublishOptions, QueueDeclareOptions},
        types::FieldTable,
        BasicProperties, Channel, Connection, ConnectionProperties,
    };
    use scouter_error::ScouterError;
    use scouter_types::ServerRecords;
    use tracing::{debug, error, info};

    #[derive(Clone)]
    pub struct RabbitMQProducer {
        pub config: RabbitMQConfig,
        producer: Channel,
    }

    impl RabbitMQProducer {
        pub async fn new(config: RabbitMQConfig) -> Result<Self, ScouterError> {
            let producer = RabbitMQProducer::setup_producer(&config).await?;

            Ok(RabbitMQProducer { config, producer })
        }

        async fn setup_producer(config: &RabbitMQConfig) -> Result<Channel, ScouterError> {
            info!("Setting up RabbitMQ producer");
            let conn = Connection::connect(&config.address, ConnectionProperties::default())
                .await
                .map_err(|e| ScouterError::Error(e.to_string()))?;
            let channel = conn.create_channel().await.unwrap();

            channel
                .queue_declare(
                    &config.queue,
                    QueueDeclareOptions::default(),
                    FieldTable::default(),
                )
                .await
                .map_err(|e| ScouterError::Error(e.to_string()))?;

            info!("RabbitMQ producer setup complete");
            Ok(channel)
        }

        pub async fn publish(&self, message: ServerRecords) -> Result<(), ScouterError> {
            let mut retries = self.config.max_retries;

            loop {
                match self
                    ._publish(message.clone())
                    .await
                    .map_err(|e| ScouterError::Error(e.to_string()))
                {
                    Ok(_) => break,
                    Err(e) => {
                        retries -= 1;
                        if retries == 0 {
                            return {
                                error!("Failed to send message to kafka: {:?}", e.to_string());
                                Err(ScouterError::Error(format!(
                                    "Failed to send message to kafka: {:?}",
                                    e.to_string()
                                )))
                            };
                        }
                    }
                }
            }

            Ok(())
        }

        pub async fn _publish(&self, message: ServerRecords) -> Result<(), ScouterError> {
            let serialized_msg = serde_json::to_string(&message).unwrap().into_bytes();

            debug!("Publishing message to RabbitMQ");

            self.producer
                .basic_publish(
                    "",
                    &self.config.queue,
                    BasicPublishOptions::default(),
                    &serialized_msg,
                    BasicProperties::default(),
                )
                .await
                .map_err(|e| ScouterError::Error(e.to_string()))?;

            Ok(())
        }

        pub async fn flush(&self) -> Result<(), ScouterError> {
            self.producer
                .close(0, "Normal shutdown")
                .await
                .map_err(|e| ScouterError::Error(e.to_string()))
        }
    }
}
