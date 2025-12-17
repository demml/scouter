pub mod auth;
pub mod interceptor;
pub mod message;
use crate::api::state::AppState;
use anyhow::Result;
pub use auth::*;
pub use message::*;
use std::sync::Arc;
use tonic::transport::Server;
use tonic_middleware::InterceptorFor;
use tracing::{info, instrument};
#[instrument(skip_all)]
pub async fn start_grpc_server(state: Arc<AppState>) -> Result<()> {
    let grpc_port = std::env::var("SCOUTER_GRPC_PORT").unwrap_or_else(|_| "50051".to_string());

    let addr = format!("0.0.0.0:{grpc_port}")
        .parse()
        .expect("Invalid gRPC address");

    let message_service = MessageGrpcService::new(state.clone()).into_server();
    let auth_service = AuthServiceImpl::new(state.clone()).into_service();
    let auth_interceptor = interceptor::AuthInterceptor::new(state.clone());

    info!("🚀 gRPC server started successfully on {}", addr);

    Server::builder()
        .add_service(auth_service)
        // auth interceptor for message service
        .add_service(InterceptorFor::new(
            message_service,
            auth_interceptor.clone(),
        ))
        .serve_with_shutdown(addr, crate::api::shutdown::grpc_shutdown_signal())
        .await?;

    Ok(())
}
