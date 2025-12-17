pub mod api;
use crate::api::grpc::start_grpc_server;
use crate::api::setup::ScouterSetupComponents;
use crate::api::shutdown::shutdown_signal;
use crate::api::state::AppState;
use anyhow::Context;
use api::router::create_router;
use axum::Router;
use scouter_auth::auth::AuthManager;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::{error, info, instrument, warn};

/// Create shared application state
///
/// This initializes all components once and returns the shared state
/// that both HTTP and gRPC servers will use
#[instrument(skip_all)]
pub async fn create_app_state() -> Result<Arc<AppState>, anyhow::Error> {
    let scouter_components = ScouterSetupComponents::new().await?;

    let app_state = Arc::new(AppState {
        db_pool: scouter_components.db_pool,
        task_manager: scouter_components.task_manager,
        auth_manager: AuthManager::new(
            &scouter_components.server_config.auth_settings.jwt_secret,
            &scouter_components
                .server_config
                .auth_settings
                .refresh_secret,
        ),
        config: scouter_components.server_config,
        http_consumer_tx: scouter_components.http_consumer_tx,
    });

    Ok(app_state)
}

/// Create the HTTP router with provided app state
#[instrument(skip_all)]
pub async fn create_http_router(app_state: Arc<AppState>) -> Result<Router, anyhow::Error> {
    create_router(app_state)
        .await
        .with_context(|| "Failed to create router")
}

/// Start the HTTP server with provided app state
#[instrument(skip_all)]
pub async fn start_http_server_with_state(app_state: Arc<AppState>) -> Result<(), anyhow::Error> {
    let router = create_http_router(app_state.clone()).await?;

    let port = std::env::var("SCOUTER_SERVER_PORT").unwrap_or_else(|_| "8000".to_string());
    let addr = format!("0.0.0.0:{port}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("Failed to bind HTTP server to {addr}"))?;

    info!("🚀 HTTP server started successfully on {}", addr);

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal(app_state.clone()))
        .await
        .with_context(|| "Failed to start HTTP server")?;

    Ok(())
}

/// Start both HTTP and gRPC servers with shared state
#[instrument(skip_all)]
pub async fn start_server() -> Result<(), anyhow::Error> {
    // Create shared app state once
    let app_state = create_app_state().await?;

    // Clone Arc for each server (cheap operation)
    let http_state = Arc::clone(&app_state);
    let grpc_state = Arc::clone(&app_state);

    // Start both servers concurrently
    let (http_result, grpc_result) = tokio::join!(
        start_http_server_with_state(http_state),
        start_grpc_server(grpc_state)
    );

    // Log results - both should shut down gracefully
    match &http_result {
        Ok(_) => info!("HTTP server shut down gracefully"),
        Err(e) => error!("HTTP server error: {}", e),
    }

    match &grpc_result {
        Ok(_) => info!("gRPC server shut down gracefully"),
        Err(e) => error!("gRPC server error: {}", e),
    }

    // Return error if either failed
    http_result?;
    grpc_result?;

    Ok(())
}

/// Start server in background with handle for management
#[instrument(skip_all)]
pub fn start_server_in_background() -> Arc<Mutex<Option<JoinHandle<()>>>> {
    let handle = Arc::new(Mutex::new(None));
    let handle_clone = handle.clone();

    tokio::spawn(async move {
        let server_handle = tokio::spawn(async {
            if let Err(e) = start_server().await {
                error!("Server error: {}", e);
            }
        });

        *handle_clone.lock().await = Some(server_handle);
    });

    handle
}

/// Stop the background server gracefully
pub async fn stop_server(handle: Arc<Mutex<Option<JoinHandle<()>>>>) {
    if let Some(handle) = handle.lock().await.take() {
        // Send shutdown signal (already handled by signal handlers)
        warn!("Initiating server shutdown...");

        // Wait for graceful shutdown to complete
        match tokio::time::timeout(std::time::Duration::from_secs(30), handle).await {
            Ok(Ok(())) => info!("Server stopped gracefully"),
            Ok(Err(e)) => error!("Server task panicked: {:?}", e),
            Err(_) => {
                warn!("Shutdown timeout exceeded, forcing termination");
                // Force termination after timeout
            }
        }
    }
}
