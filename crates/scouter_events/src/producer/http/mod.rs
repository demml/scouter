pub mod producer;
pub mod types;

pub use producer::{HTTPClient, HTTPProducer};
pub use types::{HTTPConfig, RequestType, Routes};
