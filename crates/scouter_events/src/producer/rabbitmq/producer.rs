#[cfg(feature = "rabbitmq")]
pub mod rabbitmq_producer {
    use crate::producer::rabbitmq::types::RabbitMQConfig;
    use lapin::{
        options::{BasicPublishOptions, QueueDeclareOptions},
        types::FieldTable,
        BasicProperties, Channel, ChannelState, Connection, ConnectionProperties,
    };
    use scouter_error::EventError;
    use scouter_types::ServerRecords;
    use tracing::{debug, error, info};

    #[derive(Clone)]
    pub struct RabbitMQProducer {
        pub config: RabbitMQConfig,
        producer: Channel,
    }

    impl RabbitMQProducer {
        pub async fn new(config: RabbitMQConfig) -> Result<Self, EventError> {
            let producer = RabbitMQProducer::setup_producer(&config).await?;

            Ok(RabbitMQProducer { config, producer })
        }

        async fn setup_producer(config: &RabbitMQConfig) -> Result<Channel, EventError> {
            info!("Setting up RabbitMQ producer");
            let conn = Connection::connect(&config.address, ConnectionProperties::default())
                .await
                .map_err(EventError::traced_connection_error)?;
            let channel = conn
                .create_channel()
                .await
                .map_err(EventError::traced_channel_error)?;

            channel
                .queue_declare(
                    &config.queue,
                    QueueDeclareOptions::default(),
                    FieldTable::default(),
                )
                .await
                .map_err(EventError::traced_declare_queue_error)?;

            info!("RabbitMQ producer setup complete");
            Ok(channel)
        }

        pub async fn publish(&self, message: ServerRecords) -> Result<(), EventError> {
            let mut retries = self.config.max_retries;

            loop {
                match self._publish(message.clone()).await {
                    Ok(_) => break,
                    Err(e) => {
                        retries -= 1;
                        if retries == 0 {
                            return Err(e);
                        }
                    }
                }
            }

            Ok(())
        }

        pub async fn _publish(&self, message: ServerRecords) -> Result<(), EventError> {
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
                .map_err(EventError::traced_publish_error)?;

            Ok(())
        }

        pub async fn flush(&self) -> Result<(), EventError> {
            let status = self.producer.status().state();

            match status {
                ChannelState::Closed => {
                    info!("RabbitMQ producer channel is closed");
                    Ok(())
                }
                ChannelState::Closing => {
                    error!("RabbitMQ producer channel is closing");
                    Ok(())
                }
                _ => {
                    self.producer
                        .close(0, "Normal shutdown")
                        .await
                        .map_err(EventError::traced_flush_error)?;
                    Ok(())
                }
            }
        }
    }
}
