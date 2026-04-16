pub mod api;
use crate::api::grpc::start_grpc_server;
use crate::api::setup::ScouterSetupComponents;
use crate::api::shutdown::shutdown_signal;
use crate::api::state::AppState;
use anyhow::Context;
use api::router::create_router;
use axum::Router;
use clap::ValueEnum;
use scouter_auth::auth::AuthManager;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::{error, info, instrument, warn};

#[derive(Clone, Debug, Default, ValueEnum)]
pub enum ServeMode {
    /// Run both HTTP and gRPC servers (default)
    #[default]
    Both,
    /// Run only the HTTP server
    Http,
    /// Run only the gRPC server
    Grpc,
}

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
        server_record_tx: scouter_components.server_record_tx,
        trace_record_tx: scouter_components.trace_record_tx,
        tag_record_tx: scouter_components.tag_record_tx,
        trace_service: scouter_components.trace_service,
        trace_summary_service: scouter_components.trace_summary_service,
        dataset_manager: scouter_components.dataset_manager,
        eval_scenario_service: scouter_components.eval_scenario_service,
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

#[instrument(skip_all)]
pub async fn start_server_with_mode(mode: ServeMode) -> Result<(), anyhow::Error> {
    let app_state = create_app_state().await?;

    match mode {
        ServeMode::Http => {
            start_http_server_with_state(app_state).await?;
            info!("HTTP server shut down gracefully");
        }
        ServeMode::Grpc => {
            start_grpc_server(app_state).await?;
            info!("gRPC server shut down gracefully");
        }
        ServeMode::Both => {
            let (http_result, grpc_result) = tokio::join!(
                start_http_server_with_state(Arc::clone(&app_state)),
                start_grpc_server(Arc::clone(&app_state))
            );

            match &http_result {
                Ok(_) => info!("HTTP server shut down gracefully"),
                Err(e) => error!("HTTP server error: {}", e),
            }
            match &grpc_result {
                Ok(_) => info!("gRPC server shut down gracefully"),
                Err(e) => error!("gRPC server error: {}", e),
            }

            http_result?;
            grpc_result?;
        }
    }

    Ok(())
}

#[instrument(skip_all)]
pub async fn start_server() -> Result<(), anyhow::Error> {
    start_server_with_mode(ServeMode::Both).await
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
