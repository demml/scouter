pub mod api;

use crate::api::shutdown::shutdown_signal;
use crate::api::state::AppState;
use anyhow::Context;
use api::router::create_router;
use api::setup::setup_components;
use axum::Router;
use scouter_auth::auth::AuthManager;
use std::sync::Arc;
use tracing::info;

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

    let (config, db_client, shutdown_tx) = setup_components().await?;

    let app_state = Arc::new(AppState {
        db: db_client,
        shutdown_tx,
        auth_manager: AuthManager::new(
            &config.auth_settings.jwt_secret,
            &config.auth_settings.refresh_secret,
        ),
        config,
    });

    let router = create_router(app_state.clone())
        .await
        .with_context(|| "Failed to create router")?;

    Ok((router, app_state))
}

/// Start the main server
pub async fn start_main_server() -> Result<(), anyhow::Error> {
    let (router, app_state) = create_app().await?;

    let port = std::env::var("OPSML_SERVER_PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(addr.clone())
        .await
        .with_context(|| "Failed to bind to port 8000")?;

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
