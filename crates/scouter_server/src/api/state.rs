use crate::api::{error::ServerError, task_manager::TaskManager};
use flume::Sender;
use mini_moka::sync::Cache;
use scouter_auth::auth::AuthManager;
use scouter_settings::ScouterServerConfig;
use scouter_sql::sql::traits::ProfileSqlLogic;
use scouter_sql::PostgresClient;
use scouter_types::MessageRecord;
use sqlx::{Pool, Postgres};
use std::sync::Arc;

pub struct AppState {
    pub db_pool: Pool<Postgres>,
    pub auth_manager: AuthManager,
    pub task_manager: TaskManager,
    pub config: Arc<ScouterServerConfig>,
    pub http_consumer_tx: Sender<MessageRecord>,
    pub profile_id_cache: Cache<String, i32>,
}

impl AppState {
    /// Shutdown the application gracefully
    pub async fn shutdown(&self) {
        self.task_manager.shutdown().await;
        self.db_pool.close().await;
    }

    /// Get profile ID from UID with caching
    pub async fn get_profile_id_from_uid(&self, uid: &str) -> Result<i32, ServerError> {
        match self.profile_id_cache.get(uid) {
            Some(cached_id) => Ok(cached_id),
            None => {
                let profile_id = PostgresClient::get_entity_id_from_uid(&self.db_pool, uid).await?;
                self.profile_id_cache.insert(uid.to_string(), profile_id);
                Ok(profile_id)
            }
        }
    }
}
