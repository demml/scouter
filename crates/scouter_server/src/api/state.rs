use scouter_auth::auth::AuthManager;
use scouter_sql::PostgresClient;
use tokio::sync::watch;

pub struct AppState {
    pub db: PostgresClient,
    pub auth_manager: AuthManager,
    pub shutdown_tx: watch::Sender<()>,
}
