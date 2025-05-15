use crate::api::error::ServerError;
use scouter_drift::DriftExecutor;
use scouter_settings::PollingSettings;
use sqlx::{Pool, Postgres};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, span, Instrument, Level};

pub struct BackgroundDriftManager {
    pub workers: Vec<JoinHandle<()>>,
}

impl BackgroundDriftManager {
    pub async fn start_workers(
        db_pool: &Pool<Postgres>,
        poll_settings: &PollingSettings,
        shutdown_rx: watch::Receiver<()>,
    ) -> Result<(), ServerError> {
        let num_workers = poll_settings.num_workers;
        let mut workers = Vec::with_capacity(num_workers);

        for id in 0..num_workers {
            let shutdown_rx = shutdown_rx.clone();
            let drift_executor = DriftExecutor::new(db_pool);
            let worker_shutdown_rx = shutdown_rx.clone();

            workers.push(tokio::spawn(Self::start_worker(
                id,
                drift_executor,
                worker_shutdown_rx,
            )));

            // sleep for a bit to stagger the start of the workers
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }

        debug!("✅ Started {} drift workers", num_workers);

        Ok(())
    }

    async fn start_worker(
        id: usize,
        mut executor: DriftExecutor,
        mut shutdown: watch::Receiver<()>, // Accept receiver
    ) {
        loop {
            tokio::select! {
                _ = shutdown.changed() => {
                    info!("Drift executor {}: Shutting down", id);
                    break;
                }
                result = executor.poll_for_tasks().instrument(span!(Level::INFO, "Poll")) => {
                    if let Err(e) = result {
                        error!("Alert poller error: {:?}", e);
                    }
                }
            }
        }
    }
}
