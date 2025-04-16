use chrono::{Duration, Utc};
use scouter_error::EventError;
/// Functionality for persisting data from postgres to long-term storage
use scouter_settings::{DatabaseSettings, ObjectStorageSettings};
use scouter_sql::PostgresClient;
use sqlx::{Pool, Postgres};
use tokio::sync::watch;
use tokio::task::JoinHandle;
pub struct DataManager {
    /// handler for background tasks
    pub workers: Vec<JoinHandle<()>>,
}

impl DataManager {
    pub async fn start_workers(
        pool: &Pool<Postgres>,
        db_settings: &DatabaseSettings,
        shutdown_rx: watch::Receiver<()>,
    ) -> Result<(), EventError> {
        let mut workers = Vec::with_capacity(1);

        let db_client = PostgresClient::new(Some(pool.clone()), Some(db_settings)).await?;

        let shutdown_rx = shutdown_rx.clone();
        let worker_shutdown_rx = shutdown_rx.clone();

        Ok(())
    }

    async fn data_archival_worker(
        mut db_client: PostgresClient,
        mut shutdown: watch::Receiver<()>, // Accept receiver
    ) {
        let mut last_cleanup = None;

        loop {
            let now = Utc::now();
            let should_run = match last_cleanup {
                None => true, // Run immediately on first startup
                Some(last_time) => now.signed_duration_since(last_time) >= Duration::days(1),
            };
        }
    }
}
