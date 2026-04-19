use crate::api::state::AppState;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use scouter_types::{
    contracts::ScouterServerError, GenAiAgentActivityResponse, GenAiErrorBreakdownResponse,
    GenAiErrorCount, GenAiMetricsRequest, GenAiModelUsageResponse,
    GenAiOperationBreakdownResponse, GenAiSpanFilters, GenAiSpansResponse,
    GenAiTokenMetricsResponse, GenAiToolActivityResponse,
};
use serde::Deserialize;
use std::sync::Arc;
use tracing::instrument;

#[derive(Deserialize)]
pub struct AgentActivityQuery {
    pub agent_name: Option<String>,
}

#[derive(Deserialize)]
pub struct ConversationQuery {
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

#[instrument(skip_all)]
pub async fn get_token_metrics(
    State(data): State<Arc<AppState>>,
    Json(body): Json<GenAiMetricsRequest>,
) -> Result<Json<GenAiTokenMetricsResponse>, (StatusCode, Json<ScouterServerError>)> {
    let buckets = data
        .genai_service
        .query_service
        .get_token_metrics(
            body.service_name.as_deref(),
            body.start_time,
            body.end_time,
            &body.bucket_interval,
            body.operation_name.as_deref(),
            body.provider_name.as_deref(),
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::get_genai_token_metrics_error(e)),
            )
        })?;

    Ok(Json(GenAiTokenMetricsResponse { buckets }))
}

#[instrument(skip_all)]
pub async fn get_operation_breakdown(
    State(data): State<Arc<AppState>>,
    Json(body): Json<GenAiMetricsRequest>,
) -> Result<Json<GenAiOperationBreakdownResponse>, (StatusCode, Json<ScouterServerError>)> {
    let operations = data
        .genai_service
        .query_service
        .get_operation_breakdown(
            body.service_name.as_deref(),
            body.start_time,
            body.end_time,
            body.provider_name.as_deref(),
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::get_genai_operation_breakdown_error(e)),
            )
        })?;

    Ok(Json(GenAiOperationBreakdownResponse { operations }))
}

#[instrument(skip_all)]
pub async fn get_model_usage(
    State(data): State<Arc<AppState>>,
    Json(body): Json<GenAiMetricsRequest>,
) -> Result<Json<GenAiModelUsageResponse>, (StatusCode, Json<ScouterServerError>)> {
    let models = data
        .genai_service
        .query_service
        .get_model_usage(
            body.service_name.as_deref(),
            body.start_time,
            body.end_time,
            body.provider_name.as_deref(),
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::get_genai_model_usage_error(e)),
            )
        })?;

    Ok(Json(GenAiModelUsageResponse { models }))
}

#[instrument(skip_all)]
pub async fn get_agent_activity(
    State(data): State<Arc<AppState>>,
    Query(params): Query<AgentActivityQuery>,
    Json(body): Json<GenAiMetricsRequest>,
) -> Result<Json<GenAiAgentActivityResponse>, (StatusCode, Json<ScouterServerError>)> {
    let agents = data
        .genai_service
        .query_service
        .get_agent_activity(
            body.service_name.as_deref(),
            body.start_time,
            body.end_time,
            params.agent_name.as_deref(),
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::get_genai_agent_activity_error(e)),
            )
        })?;

    Ok(Json(GenAiAgentActivityResponse { agents }))
}

#[instrument(skip_all)]
pub async fn get_tool_activity(
    State(data): State<Arc<AppState>>,
    Json(body): Json<GenAiMetricsRequest>,
) -> Result<Json<GenAiToolActivityResponse>, (StatusCode, Json<ScouterServerError>)> {
    let tools = data
        .genai_service
        .query_service
        .get_tool_activity(
            body.service_name.as_deref(),
            body.start_time,
            body.end_time,
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::get_genai_tool_activity_error(e)),
            )
        })?;

    Ok(Json(GenAiToolActivityResponse { tools }))
}

#[instrument(skip_all)]
pub async fn get_error_breakdown(
    State(data): State<Arc<AppState>>,
    Json(body): Json<GenAiMetricsRequest>,
) -> Result<Json<GenAiErrorBreakdownResponse>, (StatusCode, Json<ScouterServerError>)> {
    let raw_errors = data
        .genai_service
        .query_service
        .get_error_breakdown(
            body.service_name.as_deref(),
            body.start_time,
            body.end_time,
            body.operation_name.as_deref(),
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::get_genai_error_breakdown_error(e)),
            )
        })?;

    let errors = raw_errors
        .into_iter()
        .map(|(error_type, count)| GenAiErrorCount { error_type, count })
        .collect();

    Ok(Json(GenAiErrorBreakdownResponse { errors }))
}

#[instrument(skip_all)]
pub async fn get_genai_spans(
    State(data): State<Arc<AppState>>,
    Json(body): Json<GenAiSpanFilters>,
) -> Result<Json<GenAiSpansResponse>, (StatusCode, Json<ScouterServerError>)> {
    let spans = data
        .genai_service
        .query_service
        .get_genai_spans(&body)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::get_genai_spans_error(e)),
            )
        })?;

    Ok(Json(GenAiSpansResponse { spans }))
}

#[instrument(skip_all)]
pub async fn get_conversation_spans(
    State(data): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(params): Query<ConversationQuery>,
) -> Result<Json<GenAiSpansResponse>, (StatusCode, Json<ScouterServerError>)> {
    if id.len() > 256 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ScouterServerError::new("conversation_id exceeds maximum length".to_string())),
        ));
    }

    let start_time = params.start_time.as_deref().map(|s| {
        chrono::DateTime::parse_from_rfc3339(s)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|_| (
                StatusCode::BAD_REQUEST,
                Json(ScouterServerError::new("Invalid start_time: expected RFC3339 format".to_string())),
            ))
    }).transpose()?;

    let end_time = params.end_time.as_deref().map(|s| {
        chrono::DateTime::parse_from_rfc3339(s)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|_| (
                StatusCode::BAD_REQUEST,
                Json(ScouterServerError::new("Invalid end_time: expected RFC3339 format".to_string())),
            ))
    }).transpose()?;

    let spans = data
        .genai_service
        .query_service
        .get_conversation_spans(&id, start_time, end_time)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::get_genai_conversation_error(e)),
            )
        })?;

    Ok(Json(GenAiSpansResponse { spans }))
}

pub fn get_genai_router(prefix: &str) -> Router<Arc<AppState>> {
    Router::new()
        .route(
            &format!("{prefix}/genai/metrics/tokens"),
            post(get_token_metrics),
        )
        .route(
            &format!("{prefix}/genai/metrics/operations"),
            post(get_operation_breakdown),
        )
        .route(
            &format!("{prefix}/genai/metrics/models"),
            post(get_model_usage),
        )
        .route(
            &format!("{prefix}/genai/metrics/agents"),
            post(get_agent_activity),
        )
        .route(
            &format!("{prefix}/genai/metrics/tools"),
            post(get_tool_activity),
        )
        .route(
            &format!("{prefix}/genai/metrics/errors"),
            post(get_error_breakdown),
        )
        .route(&format!("{prefix}/genai/spans"), post(get_genai_spans))
        .route(
            &format!("{prefix}/genai/conversation/{{id}}"),
            get(get_conversation_spans),
        )
}
