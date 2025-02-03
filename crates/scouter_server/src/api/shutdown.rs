
use std::sync::atomic::Ordering;
use tokio::signal;
use tracing::info;
use crate::api::state::AppState;
use std::sync::Arc;

pub async fn shutdown_signal(app_state: Arc<AppState>) {
   

    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };


    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();


    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    // Set the shutdown flag and log the shutdown message
    app_state.shutdown.store(true, Ordering::Relaxed);
    info!("Shutdown signal received, shutting down gracefully...")
}