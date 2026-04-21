use crate::api::error::ServerError;
use chrono::Duration;
use scouter_drift::genai::TraceEvalPoller;
use scouter_settings::TraceEvalPollerSettings;
use sqlx::{Pool, Postgres};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

pub struct BackgroundTraceEvalManager;

impl BackgroundTraceEvalManager {
    pub async fn start_workers(
        db_pool: &Pool<Postgres>,
        poll_settings: &TraceEvalPollerSettings,
        shutdown_rx: watch::Receiver<()>,
    ) -> Result<(), ServerError> {
        let num_workers = poll_settings.num_workers.min(32);
        if num_workers < poll_settings.num_workers {
            warn!("TRACE_EVAL_WORKER_COUNT capped at 32 (was {})", poll_settings.num_workers);
        }
        info!("Starting {} trace eval poller workers", num_workers);

        for id in 0..num_workers {
            let poller = TraceEvalPoller::new(
                db_pool,
                Duration::seconds(poll_settings.lookback_secs as i64),
                std::time::Duration::from_secs(poll_settings.poll_interval_secs),
            );
            let shutdown_rx = shutdown_rx.clone();
            let token = CancellationToken::new();
            let token_clone = token.clone();

            tokio::spawn(async move {
                let mut rx = shutdown_rx;
                let _ = rx.changed().await;
                token_clone.cancel();
            });

            tokio::spawn(async move {
                poller.poll_for_tasks(token).await;
                debug!("Trace eval poller worker {} exited", id);
            });

            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }

        debug!("✅ Started {} trace eval poller workers", num_workers);

        Ok(())
    }
}
