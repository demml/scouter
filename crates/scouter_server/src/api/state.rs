use scouter_auth::auth::AuthManager;
use scouter_settings::ScouterServerConfig;
use sqlx::{Pool, Postgres};
use std::sync::Arc;
use tokio::sync::watch;

pub struct AppState {
    pub db_pool: Pool<Postgres>,
    pub auth_manager: AuthManager,
    pub shutdown_tx: watch::Sender<()>,
    pub config: ScouterServerConfig,
}
