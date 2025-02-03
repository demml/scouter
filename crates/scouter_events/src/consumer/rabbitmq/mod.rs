#[cfg(all(feature = "rabbitmq", feature = "sql"))]
pub mod consumer;


#[cfg(all(feature = "rabbitmq", feature = "sql"))]
pub use consumer::rabbitmq_consumer::RabbitMQConsumerManager;

