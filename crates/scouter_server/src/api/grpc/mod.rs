pub mod auth;
pub mod dataset;
pub mod eval_scenario;
pub mod interceptor;
pub mod message;
use crate::api::state::AppState;
use anyhow::Result;
pub use auth::*;
pub use dataset::*;
pub use eval_scenario::*;
pub use message::*;
use scouter_tonic::AuthServiceServer;
use scouter_tonic::DatasetServiceServer;
use scouter_tonic::EvalScenarioServiceServer;
use scouter_tonic::MessageServiceServer;
use std::sync::Arc;
use tonic::transport::Server;
use tonic_health::server::health_reporter;
use tonic_middleware::InterceptorFor;
use tonic_reflection::server::Builder;
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
    let dataset_service = DatasetGrpcService::new(state.clone()).into_server();
    let eval_scenario_service = EvalScenarioGrpcService::new(state.clone()).into_server();
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
    health_reporter
        .set_serving::<DatasetServiceServer<DatasetGrpcService>>()
        .await;
    health_reporter
        .set_serving::<EvalScenarioServiceServer<EvalScenarioGrpcService>>()
        .await;

    let reflection_service = Builder::configure()
        .register_encoded_file_descriptor_set(scouter_tonic::FILE_DESCRIPTOR_SET)
        .register_encoded_file_descriptor_set(tonic_health::pb::FILE_DESCRIPTOR_SET)
        .build_v1()
        .expect("Failed to build gRPC reflection service");

    info!("🚀 gRPC server started successfully on {}", addr);

    Server::builder()
        .add_service(health_service)
        .add_service(reflection_service)
        .add_service(auth_service)
        .add_service(InterceptorFor::new(
            message_service,
            auth_interceptor.clone(),
        ))
        .add_service(InterceptorFor::new(
            dataset_service,
            auth_interceptor.clone(),
        ))
        .add_service(InterceptorFor::new(
            eval_scenario_service,
            auth_interceptor.clone(),
        ))
        .serve_with_shutdown(addr, crate::api::shutdown::grpc_shutdown_signal())
        .await?;

    Ok(())
}
