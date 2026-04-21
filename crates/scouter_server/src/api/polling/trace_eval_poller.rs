use crate::api::error::ServerError;
use chrono::Duration;
use scouter_drift::genai::TraceEvalPoller;
use scouter_settings::TraceEvalPollerSettings;
use sqlx::{Pool, Postgres};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

pub struct BackgroundTraceEvalManager {
    pub workers: Vec<JoinHandle<()>>,
}

impl BackgroundTraceEvalManager {
    pub async fn start_workers(
        db_pool: &Pool<Postgres>,
        poll_settings: &TraceEvalPollerSettings,
        shutdown_rx: watch::Receiver<()>,
    ) -> Result<(), ServerError> {
        let num_workers = poll_settings.num_workers.min(32);
        if num_workers < poll_settings.num_workers {
            warn!(
                "TRACE_EVAL_WORKER_COUNT capped at 32 (was {})",
                poll_settings.num_workers
            );
        }
        info!("Starting {} trace eval poller workers", num_workers);

        let mut workers = Vec::with_capacity(num_workers);

        let poll_interval = std::time::Duration::from_secs(poll_settings.poll_interval_secs);

        for id in 0..num_workers {
            let poller = TraceEvalPoller::new(
                db_pool,
                Duration::seconds(poll_settings.lookback_secs as i64),
            );
            let worker_shutdown_rx = shutdown_rx.clone();

            workers.push(tokio::spawn(Self::start_worker(
                id,
                poller,
                poll_interval,
                worker_shutdown_rx,
            )));

            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }

        debug!("✅ Started {} trace eval poller workers", num_workers);

        Ok(())
    }

    async fn start_worker(
        id: usize,
        poller: TraceEvalPoller,
        poll_interval: std::time::Duration,
        mut shutdown: watch::Receiver<()>,
    ) {
        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    info!("Trace eval poller worker {}: Shutting down", id);
                    break;
                }
                _ = tokio::time::sleep(poll_interval) => {
                    if let Err(e) = poller.poll_for_tasks().await {
                        error!("Trace eval poller worker {} error: {:?}", id, e);
                    }
                }
            }
        }
    }
}
