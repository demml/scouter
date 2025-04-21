use scouter_auth::auth::AuthManager;
use scouter_settings::ScouterServerConfig;
use scouter_sql::PostgresClient;
use std::sync::Arc;
use tokio::sync::watch;

pub struct AppState {
    pub db: Arc<PostgresClient>,
    pub auth_manager: AuthManager,
    pub shutdown_tx: watch::Sender<()>,
    pub config: ScouterServerConfig,
}
