pub mod api;

use crate::api::middleware::metrics::metrics_app;
use crate::api::shutdown::shutdown_metric_signal;
use anyhow::Context;
use clap::Parser;
use mimalloc::MiMalloc;
use scouter_server::{start_server_with_mode, ServeMode};
use tracing::{info, instrument};

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[derive(Parser)]
#[command(about = "Scouter server", long_about = None)]
struct Cli {
    /// Server mode: run both HTTP and gRPC (default), HTTP only, or gRPC only
    #[arg(long, value_enum, default_value_t = ServeMode::Both)]
    mode: ServeMode,
}

#[instrument(skip_all)]
async fn start_metrics_server() -> Result<(), anyhow::Error> {
    let app = metrics_app().with_context(|| "Failed to setup metrics app")?;

    let port: usize = std::env::var("SCOUTER_SERVER_PORT")
        .unwrap_or_else(|_| "8000".to_string())
        .parse()
        .with_context(|| "Failed to parse SCOUTER_SERVER_PORT")?;

    let addr = format!("0.0.0.0:{}", port + 1);

    info!("Starting metrics server on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| "Failed to bind metrics server")?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_metric_signal())
        .await
        .with_context(|| "Failed to start metrics server")?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();
    let (_main_server, _metrics_server) =
        tokio::join!(start_server_with_mode(cli.mode), start_metrics_server());
    Ok(())
}
