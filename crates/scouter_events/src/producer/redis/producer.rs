#[cfg(feature = "redis_events")]
pub mod redis_producer {
    use crate::error::EventError;
    use crate::producer::redis::RedisConfig;
    use redis::aio::{MultiplexedConnection, PubSub};
    use redis::AsyncCommands;
    use redis::Client;
    use scouter_types::MessageRecord;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tracing::debug;

    pub struct RedisMessageBroker {
        client: Client,
    }

    impl RedisMessageBroker {
        pub fn new(redis_url: &str) -> Result<Self, EventError> {
            let client = Client::open(redis_url).map_err(EventError::RedisError)?;
            Ok(Self { client })
        }

        pub async fn get_async_connection(&self) -> Result<MultiplexedConnection, EventError> {
            self.client
                .get_multiplexed_async_connection()
                .await
                .map_err(EventError::RedisError)
        }

        pub async fn get_pub_sub(&self) -> Result<PubSub, EventError> {
            self.client
                .get_async_pubsub()
                .await
                .map_err(EventError::RedisError)
        }
    }

    #[derive(Clone)]
    pub struct RedisProducer {
        // we dont want to make publish take a &mut self, so we use a mutex here
        pub connection: Arc<Mutex<MultiplexedConnection>>,
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
                connection: Arc::new(Mutex::new(broker.get_async_connection().await?)),
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
        pub async fn publish(&self, message: MessageRecord) -> Result<(), EventError> {
            let mut retries = self.max_retries;

            loop {
                match self._publish(&message).await {
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
        pub async fn _publish(&self, message: &MessageRecord) -> Result<(), EventError> {
            let serialized_msg = serde_json::to_string(message).unwrap().into_bytes();

            debug!("Publishing message to Redis");
            let mut conn = self.connection.lock().await;

            let _: () = conn
                .publish(&self.channel, &serialized_msg)
                .await
                .map_err(EventError::RedisError)?;
            Ok(())
        }

        pub async fn flush(&self) -> Result<(), EventError> {
            Ok(())
        }
    }
}
