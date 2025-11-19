use crate::api::state::AppState;
use anyhow::{Context, Result};
use axum::{extract::State, http::StatusCode, routing::post, Extension, Json, Router};
use scouter_auth::permission::UserPermissions;
use scouter_types::{MessageRecord, ScouterResponse, ScouterServerError};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use tracing::instrument;

#[instrument(skip_all)]
pub async fn insert_message(
    State(data): State<Arc<AppState>>,
    Extension(perms): Extension<UserPermissions>,
    Json(body): Json<MessageRecord>,
) -> Result<Json<ScouterResponse>, (StatusCode, Json<ScouterServerError>)> {
    if !perms.has_write_permission(&body.space()) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ScouterServerError::permission_denied()),
        ));
    }

    match data.http_consumer_tx.send_async(body).await {
        Ok(_) => Ok(Json(ScouterResponse {
            status: "success".to_string(),
            message: "Message queued for processing".to_string(),
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ScouterServerError::new(format!(
                "Failed to enqueue message: {e:?}"
            ))),
        )),
    }
}

pub async fn get_message_router(prefix: &str) -> Result<Router<Arc<AppState>>> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        Router::new().route(&format!("{prefix}/message"), post(insert_message))
    }));

    match result {
        Ok(router) => Ok(router),
        Err(_) => {
            // panic
            Err(anyhow::anyhow!("Failed to create message router"))
                .context("Panic occurred while creating the router")
        }
    }
}
