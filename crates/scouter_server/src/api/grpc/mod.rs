pub mod auth;
pub mod interceptor;
pub mod message;
use crate::api::state::AppState;
use anyhow::Result;
pub use auth::*;
pub use message::*;
use scouter_tonic::AuthServiceServer;
use scouter_tonic::MessageServiceServer;
use std::sync::Arc;
use tonic::transport::Server;
use tonic_health::server::health_reporter;
use tonic_middleware::InterceptorFor;
use tracing::{info, instrument};

#[instrument(skip_all)]
pub async fn start_grpc_server(state: Arc<AppState>) -> Result<()> {
    let grpc_port = std::env::var("SCOUTER_GRPC_PORT").unwrap_or_else(|_| "50051".to_string());

    let addr = format!("0.0.0.0:{grpc_port}")
        .parse()
        .expect("Invalid gRPC address");

    // Create health reporter
    let (health_reporter, health_service) = health_reporter();

    // Create services
    let message_service = MessageGrpcService::new(state.clone()).into_server();
    let auth_service = AuthServiceImpl::new(state.clone()).into_service();

    // Create auth interceptor
    let auth_interceptor = interceptor::AuthInterceptor::new(state.clone());

    // Mark services as serving
    health_reporter
        .set_serving::<MessageServiceServer<MessageGrpcService>>()
        .await;
    health_reporter
        .set_serving::<AuthServiceServer<AuthServiceImpl>>()
        .await;

    info!("🚀 gRPC server started successfully on {}", addr);

    Server::builder()
        .add_service(health_service) // Health service for readiness checks
        .add_service(auth_service) // Auth service without interceptor
        .add_service(InterceptorFor::new(
            message_service,
            auth_interceptor.clone(), // Auth interceptor for message service
        ))
        .serve_with_shutdown(addr, crate::api::shutdown::grpc_shutdown_signal())
        .await?;

    Ok(())
}
