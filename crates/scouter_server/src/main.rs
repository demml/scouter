pub mod api;

use crate::api::middleware::metrics::metrics_app;
use crate::api::shutdown::shutdown_metric_signal;
use anyhow::Context;
use scouter_server::start_server;
use tracing::info;

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

/// Start the metrics server for prometheus
async fn start_metrics_server() -> Result<(), anyhow::Error> {
    let app = metrics_app().with_context(|| "Failed to setup metrics app")?;

    let port: usize = std::env::var("SCOUTER_SERVER_PORT")
        .unwrap_or_else(|_| "8000".to_string())
        .parse()
        .with_context(|| "Failed to parse SCOUTER_SERVER_PORT")?;

    let addr = format!("0.0.0.0:{}", port + 1); // Metric server runs on different port

    info!("Starting metrics server on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| "Failed to bind to port 3001 for metrics server")?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_metric_signal())
        .await
        .with_context(|| "Failed to start metrics server")?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let (_main_server, _metrics_server) = tokio::join!(start_server(), start_metrics_server());
    Ok(())
}
