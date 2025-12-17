use crate::api::state::AppState;
use anyhow::Result;
use scouter_types::MessageRecord;
use std::sync::Arc;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use tracing::{debug, error, info, instrument};
pub mod interceptor;
use scouter_tonic::{
    InsertMessageRequest, InsertMessageResponse, MessageService, MessageServiceServer,
};
use tonic_middleware::RequestInterceptorLayer;

#[derive(Clone)]
pub struct MessageGrpcService {
    state: Arc<AppState>,
}

impl MessageGrpcService {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    pub fn into_server(self) -> MessageServiceServer<Self> {
        MessageServiceServer::new(self)
    }
}

#[tonic::async_trait]
impl MessageService for MessageGrpcService {
    #[instrument(skip_all)]
    async fn insert_message(
        &self,
        request: Request<InsertMessageRequest>,
    ) -> Result<Response<InsertMessageResponse>, Status> {
        let message_bytes = &request.get_ref().message_record;

        // Check if token was refreshed and add to response
        let refreshed_token = request.metadata().get("x-refreshed-token").cloned();

        let message_record: MessageRecord = serde_json::from_slice(message_bytes).map_err(|e| {
            error!(error = %e, "Failed to deserialize MessageRecord");
            Status::invalid_argument(format!("Invalid message format: {e}"))
        })?;

        self.state
            .http_consumer_tx
            .send_async(message_record)
            .await
            .map_err(|e| {
                error!(error = ?e, "Failed to enqueue message");
                Status::internal(format!("Failed to enqueue message: {e:?}"))
            })?;

        debug!("Message successfully queued for processing");

        let mut response = Response::new(InsertMessageResponse {
            status: "success".to_string(),
            message: "Message queued for processing".to_string(),
        });

        // If token was refreshed, add it to response metadata
        if let Some(token) = refreshed_token {
            response.metadata_mut().insert("x-refreshed-token", token);
        }

        Ok(response)
    }
}

#[instrument(skip_all)]
pub async fn start_grpc_server(state: Arc<AppState>) -> Result<()> {
    let grpc_port = std::env::var("SCOUTER_GRPC_PORT").unwrap_or_else(|_| "50051".to_string());

    let addr = format!("0.0.0.0:{grpc_port}")
        .parse()
        .expect("Invalid gRPC address");

    let message_service = MessageGrpcService::new(state.clone()).into_server();
    let auth_interceptor = interceptor::AuthInterceptor::new(state.clone());

    info!("🚀 gRPC server started successfully on {}", addr);

    Server::builder()
        .layer(RequestInterceptorLayer::new(auth_interceptor.clone()))
        .add_service(message_service)
        .serve_with_shutdown(addr, crate::api::shutdown::grpc_shutdown_signal())
        .await?;

    Ok(())
}
