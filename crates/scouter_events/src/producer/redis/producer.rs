#[cfg(feature = "redis_events")]
pub mod redis_producer {
    use crate::producer::redis::RedisConfig;
    use redis::aio::{MultiplexedConnection, PubSub};
    use redis::AsyncCommands;
    use redis::Client;
    use scouter_error::EventError;
    use scouter_types::ServerRecords;
    use tracing::debug;

    pub struct RedisMessageBroker {
        client: Client,
    }

    impl RedisMessageBroker {
        pub fn new(redis_url: &str) -> Result<Self, EventError> {
            let client =
                Client::open(redis_url).map_err(|e| EventError::RedisOpenError(e.to_string()))?;
            Ok(Self { client })
        }

        pub async fn get_async_connection(&self) -> Result<MultiplexedConnection, EventError> {
            self.client
                .get_multiplexed_async_connection()
                .await
                .map_err(|e| EventError::RedisConnectionError(e.to_string()))
        }

        pub async fn get_pub_sub(&self) -> Result<PubSub, EventError> {
            self.client
                .get_async_pubsub()
                .await
                .map_err(|e| EventError::RedisPubSubError(e.to_string()))
        }
    }

    #[derive(Clone)]
    pub struct RedisProducer {
        pub connection: MultiplexedConnection,
        pub channel: String,
        pub max_retries: usize,
    }

    impl RedisProducer {
        /// Create a new Redis producer
        /// This function creates a new Redis producer that will publish messages to the specified channel
        ///
        /// # Arguments
        /// * `config` - The Redis settings
        ///
        /// # Returns
        /// * `Result<RedisProducer, EventError>` - The result of the operation
        pub async fn new(config: RedisConfig) -> Result<Self, EventError> {
            let broker = RedisMessageBroker::new(&config.address)?;
            Ok(Self {
                connection: broker.get_async_connection().await?,
                channel: config.channel.clone(),
                max_retries: 3,
            })
        }

        /// Core publish method used to publish messages to Redis
        /// This function publishes messages to Redis. It will retry the specified number of times if the publish fails
        ///
        /// # Arguments
        /// * `message` - The message to publish
        ///
        /// # Returns
        /// * `Result<(), EventError>` - The result of the operation
        pub async fn publish(&mut self, message: ServerRecords) -> Result<(), EventError> {
            let mut retries = self.max_retries;

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

        /// Async publish to Redis
        pub async fn _publish(&mut self, message: ServerRecords) -> Result<(), EventError> {
            let serialized_msg = serde_json::to_string(&message).unwrap().into_bytes();

            debug!("Publishing message to Redis");
            let _: () = self
                .connection
                .publish(&self.channel, &serialized_msg)
                .await
                .map_err(EventError::traced_publish_error)?;
            Ok(())
        }

        pub async fn flush(&self) -> Result<(), EventError> {
            Ok(())
        }
    }
}
