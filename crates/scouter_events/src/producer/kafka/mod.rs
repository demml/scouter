#[cfg(feature = "kafka")]
pub mod producer;

pub mod types;

#[cfg(feature = "kafka")]
pub use producer::kafka_producer::*;

pub use types::KafkaConfig;
