#[cfg(feature = "redis_events")]
pub mod redis_producer {
    use redis::aio::PubSub;
    use redis::{Client, Msg, RedisResult};
    use scouter_error::EventError;

    pub struct RedisMessageBroker {
        client: Client,
    }

    impl RedisMessageBroker {
        pub fn new(redis_url: &str) -> Result<Self, EventError> {
            let client =
                Client::open(redis_url).map_err(|e| EventError::RedisOpenError(e.to_string()))?;
            Ok(Self { client })
        }

        pub async fn get_pub_sub(&self) -> Result<PubSub, EventError> {
            self.client
                .get_async_pubsub()
                .await
                .map_err(|e| EventError::RedisPubSubError(e.to_string()))
        }
    }
}
