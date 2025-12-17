// ScouterProducer expects an async client

use crate::error::EventError;
use reqwest::header;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client, Response};
use scouter_settings::HttpConfig;
use scouter_tonic::GrpcClient;
use scouter_types::{JwtToken, MessageRecord, RequestType, Routes};
use serde_json::Value;
use std::sync::Arc;
// Using tokio RwLock for async read/write access since this is run within a spawned task
use tokio::sync::RwLock;
use tracing::{debug, instrument};
const TIMEOUT_SECS: u64 = 60;

#[derive(Debug, Clone)]
pub struct GrpcProducer {
    client: GrpcClient,
}

impl GrpcProducer {
    pub async fn new(config: GrpcConfig) -> Result<Self, EventError> {
        let client = GrpcClient::new(config).await?;
        Ok(GrpcProducer { client })
    }

    pub async fn publish(&self, message: MessageRecord) -> Result<(), EventError> {
        let serialized_msg: Value = serde_json::to_value(&message)?;

        let response = self
            .client
            .request(
                Routes::Message,
                RequestType::Post,
                Some(serialized_msg),
                None,
                None,
            )
            .await?;

        debug!("Published message to drift with response: {:?}", response);

        Ok(())
    }

    pub async fn flush(&self) -> Result<(), EventError> {
        Ok(())
    }
}
