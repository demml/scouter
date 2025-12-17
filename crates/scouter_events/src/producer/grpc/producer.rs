// ScouterProducer expects an async client

use crate::error::EventError;
use scouter_settings::grpc::GrpcConfig;
use scouter_tonic::GrpcClient;
use scouter_types::MessageRecord;

use tracing::debug;
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

    pub async fn publish(&mut self, message: MessageRecord) -> Result<(), EventError> {
        let msg_bytes = serde_json::to_vec(&message)?;
        let response = self.client.insert_message(msg_bytes).await?;

        debug!("Published message to drift with response: {:?}", response);

        Ok(())
    }

    pub async fn flush(&self) -> Result<(), EventError> {
        Ok(())
    }
}
