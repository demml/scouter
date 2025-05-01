#[cfg(all(feature = "redis_events", feature = "sql"))]
pub mod consumer;

#[cfg(all(feature = "redis_events", feature = "sql"))]
pub use consumer::redis_consumer::RedisConsumerManager;
