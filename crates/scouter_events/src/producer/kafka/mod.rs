#[cfg(feature = "kafka")]
pub mod producer;

#[cfg(feature = "kafka")]
pub mod types;

#[cfg(feature = "kafka")]
pub use producer::kafka_producer::*;

