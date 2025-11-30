use crate::api::{error::ServerError, task_manager::TaskManager};
use flume::Sender;
use mini_moka::sync::Cache;
use scouter_auth::auth::AuthManager;
use scouter_settings::ScouterServerConfig;
use scouter_sql::sql::{cache::EntityCache, traits::ProfileSqlLogic};
use scouter_sql::PostgresClient;
use scouter_types::contracts::ScouterServerError;
use scouter_types::MessageRecord;
use sqlx::{Pool, Postgres};
use std::sync::Arc;

use axum::http::StatusCode;

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
    pub async fn get_entity_id_from_uid(&self, uid: &str) -> Result<i32, ServerError> {
        self.entity_cache.get_entity_id_from_uid(uid).await
    }

    pub async fn get_entity_id_for_request(
        &self,
        uid: &str,
    ) -> Result<i32, (ServerError, Json<ScouterServerError>)> {
        let profile_id = self.get_entity_id_from_uid(uid).await.map_err(|e| {
            error!("Failed to get profile ID from UID: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::new(format!(
                    "Failed to get profile ID from UID: {e:?}"
                ))),
            )
        })?;
    }
}
