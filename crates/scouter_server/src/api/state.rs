use flume::Sender;
use scouter_auth::auth::AuthManager;
use scouter_settings::ScouterServerConfig;
use scouter_types::ServerRecords;
use sqlx::{Pool, Postgres};
use std::sync::Arc;

use crate::api::task_manager::TaskManager;

pub struct AppState {
    pub db_pool: Pool<Postgres>,
    pub auth_manager: AuthManager,
    pub task_manager: TaskManager,
    pub config: Arc<ScouterServerConfig>,
    pub http_consumer_tx: Sender<ServerRecords>,
}

impl AppState {
    /// Shutdown the application gracefully
    pub async fn shutdown(&self) {
        self.task_manager.shutdown().await;
        self.db_pool.close().await;
    }
}
