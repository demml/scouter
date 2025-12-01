use crate::api::{error::ServerError, task_manager::TaskManager};

use axum::Json;
use flume::Sender;
use scouter_auth::auth::AuthManager;
use scouter_settings::ScouterServerConfig;
use scouter_sql::sql::cache::entity_cache;
use scouter_sql::sql::cache::EntityCache;
use scouter_types::contracts::ScouterServerError;
use scouter_types::MessageRecord;
use sqlx::{Pool, Postgres};
use std::sync::Arc;
use tracing::error;

pub struct AppState {
    pub db_pool: Pool<Postgres>,
    pub auth_manager: AuthManager,
    pub task_manager: TaskManager,
    pub config: Arc<ScouterServerConfig>,
    pub http_consumer_tx: Sender<MessageRecord>,
    pub entity_cache: EntityCache,
}

impl AppState {
    /// Shutdown the application gracefully
    pub async fn shutdown(&self) {
        self.task_manager.shutdown().await;
        self.db_pool.close().await;
    }

    /// Get profile ID from UID with caching
    pub async fn get_entity_id_from_uid(&self, uid: &String) -> Result<i32, ServerError> {
        Ok(entity_cache().get_entity_id_from_uid(uid).await?)
    }

    pub async fn get_entity_id_for_request(
        &self,
        uid: &String,
    ) -> Result<i32, (ServerError, Json<ScouterServerError>)> {
        match self.get_entity_id_from_uid(uid).await {
            Ok(profile_id) => Ok(profile_id),
            Err(e) => {
                let error_msg = e.to_string();
                error!("Failed to get entity ID from UID: {:?}", e);
                Err((
                    e,
                    Json(ScouterServerError::new(format!(
                        "Failed to get entity ID from UID: {error_msg}"
                    ))),
                ))
            }
        }
    }
}
