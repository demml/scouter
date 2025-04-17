use scouter_drift::DriftExecutor;
use scouter_error::ScouterError;
use scouter_settings::{DatabaseSettings, PollingSettings};
use scouter_sql::PostgresClient;
use sqlx::Pool;
use sqlx::Postgres;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, span, Instrument, Level};

pub struct BackgroundPollManager {
    pub workers: Vec<JoinHandle<()>>,
}

impl BackgroundPollManager {
    pub async fn start_workers(
        pool: &Pool<Postgres>,
        poll_settings: &PollingSettings,
        db_settings: &DatabaseSettings,
        shutdown_rx: watch::Receiver<()>,
    ) -> Result<(), ScouterError> {
        let num_workers = poll_settings.num_workers;
        let mut workers = Vec::with_capacity(num_workers);

        for id in 0..num_workers {
            let db_client = PostgresClient::new(Some(pool.clone()), Some(db_settings)).await?;

            let shutdown_rx = shutdown_rx.clone();
            let drift_executor = DriftExecutor::new(db_client);
            let worker_shutdown_rx = shutdown_rx.clone();

            workers.push(tokio::spawn(Self::start_worker(
                id,
                drift_executor,
                worker_shutdown_rx,
            )));
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
