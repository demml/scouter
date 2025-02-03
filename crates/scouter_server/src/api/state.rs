use scouter_sql::PostgresClient;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

pub struct AppState {
    pub db: PostgresClient,
    pub shutdown: Arc<AtomicBool>,
}
