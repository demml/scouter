#[cfg(all(feature = "kafka", feature = "sql"))]
pub mod consumer;

#[cfg(all(feature = "kafka", feature = "sql"))]
pub use consumer::kafka_consumer::KafkaConsumerManager;
