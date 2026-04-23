use crate::api::state::AppState;
use anyhow::{Context, Result};
use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use metrics::counter;
use scouter_types::{MessageRecord, ScouterResponse, ScouterServerError};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use tracing::{error, instrument};

#[utoipa::path(
    post,
    path = "/scouter/message",
    request_body(content = serde_json::Value, description = "Message record (ServerRecords | TraceServerRecord | TagRecord)", content_type = "application/json"),
    responses(
        (status = 200, description = "Message queued", body = ScouterResponse),
        (status = 429, description = "Channel full, retry later", body = ScouterServerError),
        (status = 500, description = "Internal server error", body = ScouterServerError),
    ),
    tag = "messages",
    security(("bearer_token" = []))
)]
#[instrument(skip_all)]
pub async fn insert_message(
    State(data): State<Arc<AppState>>,
    Json(body): Json<MessageRecord>,
) -> Result<Json<ScouterResponse>, (StatusCode, Json<ScouterServerError>)> {
    // Map TrySendError to (is_full: bool) to unify the three typed channel arms
    let enqueue_result: Result<(), bool> = match body {
        MessageRecord::ServerRecords(r) => data
            .server_record_tx
            .try_send(r)
            .map_err(|e| matches!(e, flume::TrySendError::Full(_))),
        MessageRecord::TraceServerRecord(r) => data
            .trace_record_tx
            .try_send(r)
            .map_err(|e| matches!(e, flume::TrySendError::Full(_))),
        MessageRecord::TagServerRecord(r) => data
            .tag_record_tx
            .try_send(r)
            .map_err(|e| matches!(e, flume::TrySendError::Full(_))),
    };
    match enqueue_result {
        Ok(_) => Ok(Json(ScouterResponse {
            status: "success".to_string(),
            message: "Message queued for processing".to_string(),
        })),
        Err(true) => {
            counter!("channel_full").increment(1);
            Err((
                StatusCode::TOO_MANY_REQUESTS,
                Json(ScouterServerError::new(
                    "Service busy, retry later".to_string(),
                )),
            ))
        }
        Err(false) => {
            error!("Channel disconnected while enqueuing message");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::new(
                    "Failed to enqueue message".to_string(),
                )),
            ))
        }
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
