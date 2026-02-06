use crate::api::{error::ServerError, task_manager::TaskManager};

use axum::http::StatusCode;
use axum::Json;
use flume::Sender;
use scouter_auth::auth::AuthManager;
use scouter_settings::ScouterServerConfig;
use scouter_sql::sql::aggregator::shutdown_trace_cache;
use scouter_sql::sql::cache::entity_cache;
use scouter_types::MessageRecord;
use scouter_types::{contracts::ScouterServerError, DriftType};
use sqlx::{Pool, Postgres};
use std::sync::Arc;
use tracing::error;

pub struct AppState {
    pub db_pool: Pool<Postgres>,
    pub auth_manager: AuthManager,
    pub task_manager: TaskManager,
    pub config: Arc<ScouterServerConfig>,
    pub http_consumer_tx: Sender<MessageRecord>,
}

impl AppState {
    /// Shutdown the application gracefully
    pub async fn shutdown(&self) {
        self.task_manager.shutdown().await;
        self.db_pool.close().await;
        shutdown_trace_cache().await.unwrap_or_else(|e| {
            error!("Failed to shutdown trace cache: {:?}", e);
            0
        });
    }

    /// Get profile ID from UID with caching
    pub async fn get_entity_id_from_uid(&self, uid: &String) -> Result<i32, ServerError> {
        Ok(entity_cache()
            .get_entity_id_from_uid(&self.db_pool, uid)
            .await?)
    }

    pub async fn get_entity_id_for_request(
        &self,
        uid: &String,
    ) -> Result<i32, (StatusCode, Json<ScouterServerError>)> {
        match self.get_entity_id_from_uid(uid).await {
            Ok(profile_id) => Ok(profile_id),
            Err(e) => {
                let error_msg = e.to_string();
                error!("Failed to get entity ID from UID: {:?}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ScouterServerError::new(format!(
                        "Failed to get entity ID from UID: {error_msg}"
                    ))),
                ))
            }
        }
    }

    pub async fn get_entity_id_from_space_name_version_drift_type(
        &self,
        space: &str,
        name: &str,
        version: &str,
        drift_type: &DriftType,
    ) -> Result<i32, ServerError> {
        Ok(entity_cache()
            .get_entity_id_from_space_name_version_drift_type(
                &self.db_pool,
                space,
                name,
                version,
                drift_type,
            )
            .await?)
    }

    pub async fn get_entity_id_for_request_from_args(
        &self,
        space: &str,
        name: &str,
        version: &str,
        drift_type: &DriftType,
    ) -> Result<i32, (StatusCode, Json<ScouterServerError>)> {
        match self
            .get_entity_id_from_space_name_version_drift_type(space, name, version, drift_type)
            .await
        {
            Ok(profile_id) => Ok(profile_id),
            Err(e) => {
                let error_msg = e.to_string();
                error!(
                    "Failed to get entity ID from space, name, version, drift_type: {:?}",
                    e
                );
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ScouterServerError::new(format!(
                    "Failed to get entity ID from space, name, version, drift_type: {error_msg}"
                ))),
                ))
            }
        }
    }
}
