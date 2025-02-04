use scouter_sql::PostgresClient;
use tokio::sync::watch;

pub struct AppState {
    pub db: PostgresClient,
    pub shutdown_tx: watch::Sender<()>,
}
