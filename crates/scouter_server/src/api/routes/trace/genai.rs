use crate::api::state::AppState;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use scouter_types::{
    contracts::ScouterServerError, GenAiAgentActivity, GenAiMetricsRequest, GenAiModelUsage,
    GenAiOperationBreakdown, GenAiSpanFilters, GenAiSpanRecord, GenAiTokenBucket,
    GenAiToolActivity,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::instrument;

#[derive(Serialize)]
pub struct TokenMetricsResponse {
    pub buckets: Vec<GenAiTokenBucket>,
}

#[derive(Serialize)]
pub struct OperationBreakdownResponse {
    pub operations: Vec<GenAiOperationBreakdown>,
}

#[derive(Serialize)]
pub struct ModelUsageResponse {
    pub models: Vec<GenAiModelUsage>,
}

#[derive(Serialize)]
pub struct AgentActivityResponse {
    pub agents: Vec<GenAiAgentActivity>,
}

#[derive(Serialize)]
pub struct ToolActivityResponse {
    pub tools: Vec<GenAiToolActivity>,
}

#[derive(Serialize)]
pub struct ErrorBreakdownResponse {
    pub errors: Vec<(String, i64)>,
}

#[derive(Serialize)]
pub struct GenAiSpansResponse {
    pub spans: Vec<GenAiSpanRecord>,
}

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
) -> Result<Json<TokenMetricsResponse>, (StatusCode, Json<ScouterServerError>)> {
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

    Ok(Json(TokenMetricsResponse { buckets }))
}

#[instrument(skip_all)]
pub async fn get_operation_breakdown(
    State(data): State<Arc<AppState>>,
    Json(body): Json<GenAiMetricsRequest>,
) -> Result<Json<OperationBreakdownResponse>, (StatusCode, Json<ScouterServerError>)> {
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

    Ok(Json(OperationBreakdownResponse { operations }))
}

#[instrument(skip_all)]
pub async fn get_model_usage(
    State(data): State<Arc<AppState>>,
    Json(body): Json<GenAiMetricsRequest>,
) -> Result<Json<ModelUsageResponse>, (StatusCode, Json<ScouterServerError>)> {
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

    Ok(Json(ModelUsageResponse { models }))
}

#[instrument(skip_all)]
pub async fn get_agent_activity(
    State(data): State<Arc<AppState>>,
    Query(params): Query<AgentActivityQuery>,
    Json(body): Json<GenAiMetricsRequest>,
) -> Result<Json<AgentActivityResponse>, (StatusCode, Json<ScouterServerError>)> {
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

    Ok(Json(AgentActivityResponse { agents }))
}

#[instrument(skip_all)]
pub async fn get_tool_activity(
    State(data): State<Arc<AppState>>,
    Json(body): Json<GenAiMetricsRequest>,
) -> Result<Json<ToolActivityResponse>, (StatusCode, Json<ScouterServerError>)> {
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

    Ok(Json(ToolActivityResponse { tools }))
}

#[instrument(skip_all)]
pub async fn get_error_breakdown(
    State(data): State<Arc<AppState>>,
    Json(body): Json<GenAiMetricsRequest>,
) -> Result<Json<ErrorBreakdownResponse>, (StatusCode, Json<ScouterServerError>)> {
    let errors = data
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

    Ok(Json(ErrorBreakdownResponse { errors }))
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
    let start_time = params.start_time.as_deref().map(|s| {
        chrono::DateTime::parse_from_rfc3339(s)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|_| (
                StatusCode::BAD_REQUEST,
                Json(ScouterServerError::new(format!("Invalid start_time: {s}"))),
            ))
    }).transpose()?;

    let end_time = params.end_time.as_deref().map(|s| {
        chrono::DateTime::parse_from_rfc3339(s)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|_| (
                StatusCode::BAD_REQUEST,
                Json(ScouterServerError::new(format!("Invalid end_time: {s}"))),
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
