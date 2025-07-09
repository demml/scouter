pub mod api;

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
use tracing::error;
use tracing::{info, instrument};

/// Create the main server
///
/// This function will create the main server with the given configuration
///
/// # Arguments
///
/// * `config` - The server configuration
///
/// # Returns
///
/// The main server router
pub async fn create_app() -> Result<(Router, Arc<AppState>), anyhow::Error> {
    // setup logging, soft fail if it fails

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

    let router = create_router(app_state.clone())
        .await
        .with_context(|| "Failed to create router")?;

    Ok((router, app_state))
}

/// Start the main server
#[instrument(skip_all)]
pub async fn start_server() -> Result<(), anyhow::Error> {
    let (router, app_state) = create_app().await?;

    let port = std::env::var("SCOUTER_SERVER_PORT").unwrap_or_else(|_| "8000".to_string());
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(addr.clone())
        .await
        .with_context(|| format!("Failed to bind to port {port}"))?;

    info!(
        "🚀 Scouter Server started successfully on {:?}",
        addr.clone().to_string()
    );
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal(app_state.clone()))
        .await
        .with_context(|| "Failed to start main server")?;

    Ok(())
}

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

pub async fn stop_server(handle: Arc<Mutex<Option<JoinHandle<()>>>>) {
    if let Some(handle) = handle.lock().await.take() {
        handle.abort();
        info!("Server stopped");
    }
}
