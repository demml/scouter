#[cfg(any(feature = "kafka", feature = "kafka-vendored"))]
pub mod producer;

pub mod types;

#[cfg(any(feature = "kafka", feature = "kafka-vendored"))]
pub use producer::kafka_producer::*;

pub use types::KafkaConfig;
