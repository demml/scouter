use crate::api::state::AppState;
use anyhow::Result;
use scouter_types::MessageRecord;
use std::sync::Arc;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use tracing::{debug, error, info, instrument};

/// Generated protobuf code
pub mod proto {
    include!("../generated/scouter.message.v1.rs");
}

use proto::{
    message_service_server::{MessageService, MessageServiceServer},
    InsertMessageRequest, InsertMessageResponse,
};

/// gRPC service implementation for message insertion
#[derive(Clone)]
pub struct MessageGrpcService {
    state: Arc<AppState>,
}

impl MessageGrpcService {
    /// Create a new gRPC service instance
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    /// Convert into a Tonic server
    pub fn into_server(self) -> MessageServiceServer<Self> {
        MessageServiceServer::new(self)
    }
}

#[tonic::async_trait]
impl MessageService for MessageGrpcService {
    /// Handle single message insertion
    #[instrument(
        skip_all,
        fields(
            message_size = request.get_ref().message_record.len(),
        )
    )]
    async fn insert_message(
        &self,
        request: Request<InsertMessageRequest>,
    ) -> Result<Response<InsertMessageResponse>, Status> {
        let message_bytes = &request.get_ref().message_record;

        // Deserialize MessageRecord from JSON bytes
        let message_record: MessageRecord = serde_json::from_slice(message_bytes).map_err(|e| {
            error!(error = %e, "Failed to deserialize MessageRecord");
            Status::invalid_argument(format!("Invalid message format: {e}"))
        })?;

        // Send to processing queue (same channel as HTTP endpoint)
        self.state
            .http_consumer_tx
            .send_async(message_record)
            .await
            .map_err(|e| {
                error!(error = ?e, "Failed to enqueue message");
                Status::internal(format!("Failed to enqueue message: {e:?}"))
            })?;

        debug!("Message successfully queued for processing");

        Ok(Response::new(InsertMessageResponse {
            status: "success".to_string(),
            message: "Message queued for processing".to_string(),
        }))
    }
}

#[instrument(skip_all)]
pub async fn start_grpc_server(state: Arc<AppState>) -> Result<()> {
    let grpc_port = std::env::var("SCOUTER_GRPC_PORT").unwrap_or_else(|_| "50051".to_string());

    let addr = format!("0.0.0.0:{grpc_port}")
        .parse()
        .expect("Invalid gRPC address");

    let message_service = MessageGrpcService::new(state).into_server();

    info!("🚀 gRPC server started successfully on {}", addr);

    Server::builder()
        .add_service(message_service)
        .serve_with_shutdown(addr, crate::api::shutdown::grpc_shutdown_signal())
        .await?;

    Ok(())
}
