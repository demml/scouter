use crate::api::routes::trace::metrics::{
    build_agent_panel, build_genai_dashboard_response, build_tool_panel,
    build_trace_metrics_response, validate_bucket_interval,
};
use crate::api::state::AppState;

use axum::{
    Json, Router,
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use scouter_auth::permission::UserPermissions;
use scouter_types::{
    AgentActivityQuery, AgentDashboardRequest, AgentDashboardResponse, ConversationQuery,
    GenAiAgentActivityResponse, GenAiDashboardRequest, GenAiDashboardResponse,
    GenAiErrorBreakdownResponse, GenAiErrorCount, GenAiMetricsRequest, GenAiModelUsageResponse,
    GenAiOperationBreakdownResponse, GenAiSpanFilters, GenAiSpansResponse,
    GenAiTokenMetricsResponse, GenAiToolActivityResponse, GenAiTraceMetricsRequest,
    GenAiTraceMetricsResponse, ToolDashboardRequest, ToolDashboardResponse, TraceId,
    contracts::ScouterServerError,
};

use chrono::Utc;
use std::sync::Arc;
use tracing::instrument;

#[cfg(test)]
use scouter_types::GenAiSpanRecord;

const DEFAULT_TRACE_METRICS_WINDOW_HOURS: i64 = 24;
const MAX_TRACE_METRICS_WINDOW_DAYS: i64 = 30;
const MAX_DASHBOARD_WINDOW_DAYS: i64 = 30;
const MAX_TRACE_METRICS_SPAN_LIMIT: usize = 5_000;

#[utoipa::path(
    post,
    path = "/scouter/genai/metrics/tokens",
    request_body = GenAiMetricsRequest,
    responses(
        (status = 200, description = "Token usage metrics over time", body = GenAiTokenMetricsResponse),
        (status = 500, description = "Internal server error", body = ScouterServerError),
    ),
    tag = "genai",
    security(("bearer_token" = []))
)]
#[instrument(skip_all)]
pub async fn get_token_metrics(
    State(data): State<Arc<AppState>>,
    Json(body): Json<GenAiMetricsRequest>,
) -> Result<Json<GenAiTokenMetricsResponse>, (StatusCode, Json<ScouterServerError>)> {
    for (field_name, val) in [
        ("service_name", body.service_name.as_deref()),
        ("service_namespace", body.service_namespace.as_deref()),
        ("service_version", body.service_version.as_deref()),
        ("service_instance_id", body.service_instance_id.as_deref()),
        ("operation_name", body.operation_name.as_deref()),
        ("provider_name", body.provider_name.as_deref()),
        ("agent_name", body.agent_name.as_deref()),
        ("model", body.model.as_deref()),
    ] {
        if val.is_some_and(|v| v.len() > 256) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ScouterServerError::new(format!(
                    "{field_name} exceeds maximum length of 256 characters"
                ))),
            ));
        }
    }
    let buckets = data
        .genai_service
        .query_service
        .get_token_metrics(
            body.service_name.as_deref(),
            body.service_namespace.as_deref(),
            body.service_version.as_deref(),
            body.service_instance_id.as_deref(),
            None,
            body.start_time,
            body.end_time,
            &body.bucket_interval,
            body.operation_name.as_deref(),
            body.provider_name.as_deref(),
            body.agent_name.as_deref(),
            body.model.as_deref(),
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

#[utoipa::path(
    post,
    path = "/scouter/genai/metrics/operations",
    request_body = GenAiMetricsRequest,
    responses(
        (status = 200, description = "Operation breakdown by provider and type", body = GenAiOperationBreakdownResponse),
        (status = 500, description = "Internal server error", body = ScouterServerError),
    ),
    tag = "genai",
    security(("bearer_token" = []))
)]
#[instrument(skip_all)]
pub async fn get_operation_breakdown(
    State(data): State<Arc<AppState>>,
    Json(body): Json<GenAiMetricsRequest>,
) -> Result<Json<GenAiOperationBreakdownResponse>, (StatusCode, Json<ScouterServerError>)> {
    for (field_name, val) in [
        ("service_name", body.service_name.as_deref()),
        ("service_namespace", body.service_namespace.as_deref()),
        ("service_version", body.service_version.as_deref()),
        ("service_instance_id", body.service_instance_id.as_deref()),
        ("provider_name", body.provider_name.as_deref()),
        ("agent_name", body.agent_name.as_deref()),
        ("model", body.model.as_deref()),
    ] {
        if val.is_some_and(|v| v.len() > 256) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ScouterServerError::new(format!(
                    "{field_name} exceeds maximum length of 256 characters"
                ))),
            ));
        }
    }
    let operations = data
        .genai_service
        .query_service
        .get_operation_breakdown(
            body.service_name.as_deref(),
            body.service_namespace.as_deref(),
            body.service_version.as_deref(),
            body.service_instance_id.as_deref(),
            None,
            body.start_time,
            body.end_time,
            body.provider_name.as_deref(),
            body.agent_name.as_deref(),
            body.model.as_deref(),
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

#[utoipa::path(
    post,
    path = "/scouter/genai/metrics/models",
    request_body = GenAiMetricsRequest,
    responses(
        (status = 200, description = "Model usage statistics including token counts and latency", body = GenAiModelUsageResponse),
        (status = 500, description = "Internal server error", body = ScouterServerError),
    ),
    tag = "genai",
    security(("bearer_token" = []))
)]
#[instrument(skip_all)]
pub async fn get_model_usage(
    State(data): State<Arc<AppState>>,
    Json(body): Json<GenAiMetricsRequest>,
) -> Result<Json<GenAiModelUsageResponse>, (StatusCode, Json<ScouterServerError>)> {
    for (field_name, val) in [
        ("service_name", body.service_name.as_deref()),
        ("service_namespace", body.service_namespace.as_deref()),
        ("service_version", body.service_version.as_deref()),
        ("service_instance_id", body.service_instance_id.as_deref()),
        ("provider_name", body.provider_name.as_deref()),
        ("agent_name", body.agent_name.as_deref()),
        ("model", body.model.as_deref()),
    ] {
        if val.is_some_and(|v| v.len() > 256) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ScouterServerError::new(format!(
                    "{field_name} exceeds maximum length of 256 characters"
                ))),
            ));
        }
    }
    let models = data
        .genai_service
        .query_service
        .get_model_usage(
            body.service_name.as_deref(),
            body.service_namespace.as_deref(),
            body.service_version.as_deref(),
            body.service_instance_id.as_deref(),
            None,
            body.start_time,
            body.end_time,
            body.provider_name.as_deref(),
            body.agent_name.as_deref(),
            body.model.as_deref(),
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

#[utoipa::path(
    post,
    path = "/scouter/genai/metrics/agents",
    params(AgentActivityQuery),
    request_body = GenAiMetricsRequest,
    responses(
        (status = 200, description = "Agent activity with token usage and conversation counts", body = GenAiAgentActivityResponse),
        (status = 500, description = "Internal server error", body = ScouterServerError),
    ),
    tag = "genai",
    security(("bearer_token" = []))
)]
#[instrument(skip_all)]
pub async fn get_agent_activity(
    State(data): State<Arc<AppState>>,
    Query(params): Query<AgentActivityQuery>,
    Json(body): Json<GenAiMetricsRequest>,
) -> Result<Json<GenAiAgentActivityResponse>, (StatusCode, Json<ScouterServerError>)> {
    for (field_name, val) in [
        ("service_name", body.service_name.as_deref()),
        ("service_namespace", body.service_namespace.as_deref()),
        ("service_version", body.service_version.as_deref()),
        ("service_instance_id", body.service_instance_id.as_deref()),
        ("provider_name", body.provider_name.as_deref()),
        ("agent_name", params.agent_name.as_deref()),
    ] {
        if val.is_some_and(|v| v.len() > 256) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ScouterServerError::new(format!(
                    "{field_name} exceeds maximum length of 256 characters"
                ))),
            ));
        }
    }
    let agents = data
        .genai_service
        .query_service
        .get_agent_activity(
            body.service_name.as_deref(),
            body.service_namespace.as_deref(),
            body.service_version.as_deref(),
            body.service_instance_id.as_deref(),
            None,
            body.start_time,
            body.end_time,
            params.agent_name.as_deref(),
            body.provider_name.as_deref(),
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

#[utoipa::path(
    post,
    path = "/scouter/genai/metrics/tools",
    request_body = GenAiMetricsRequest,
    responses(
        (status = 200, description = "Tool call activity with error rates and latency", body = GenAiToolActivityResponse),
        (status = 500, description = "Internal server error", body = ScouterServerError),
    ),
    tag = "genai",
    security(("bearer_token" = []))
)]
#[instrument(skip_all)]
pub async fn get_tool_activity(
    State(data): State<Arc<AppState>>,
    Json(body): Json<GenAiMetricsRequest>,
) -> Result<Json<GenAiToolActivityResponse>, (StatusCode, Json<ScouterServerError>)> {
    for (field_name, val) in [
        ("service_name", body.service_name.as_deref()),
        ("service_namespace", body.service_namespace.as_deref()),
        ("service_version", body.service_version.as_deref()),
        ("service_instance_id", body.service_instance_id.as_deref()),
        ("agent_name", body.agent_name.as_deref()),
        ("model", body.model.as_deref()),
    ] {
        if val.is_some_and(|v| v.len() > 256) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ScouterServerError::new(format!(
                    "{field_name} exceeds maximum length of 256 characters"
                ))),
            ));
        }
    }
    let tools = data
        .genai_service
        .query_service
        .get_tool_activity(
            body.service_name.as_deref(),
            body.service_namespace.as_deref(),
            body.service_version.as_deref(),
            body.service_instance_id.as_deref(),
            None,
            body.start_time,
            body.end_time,
            body.agent_name.as_deref(),
            body.model.as_deref(),
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

#[utoipa::path(
    post,
    path = "/scouter/genai/metrics/errors",
    request_body = GenAiMetricsRequest,
    responses(
        (status = 200, description = "Error breakdown by error type with counts", body = GenAiErrorBreakdownResponse),
        (status = 500, description = "Internal server error", body = ScouterServerError),
    ),
    tag = "genai",
    security(("bearer_token" = []))
)]
#[instrument(skip_all)]
pub async fn get_error_breakdown(
    State(data): State<Arc<AppState>>,
    Json(body): Json<GenAiMetricsRequest>,
) -> Result<Json<GenAiErrorBreakdownResponse>, (StatusCode, Json<ScouterServerError>)> {
    for (field_name, val) in [
        ("service_name", body.service_name.as_deref()),
        ("service_namespace", body.service_namespace.as_deref()),
        ("service_version", body.service_version.as_deref()),
        ("service_instance_id", body.service_instance_id.as_deref()),
        ("operation_name", body.operation_name.as_deref()),
        ("agent_name", body.agent_name.as_deref()),
        ("model", body.model.as_deref()),
    ] {
        if val.is_some_and(|v| v.len() > 256) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ScouterServerError::new(format!(
                    "{field_name} exceeds maximum length of 256 characters"
                ))),
            ));
        }
    }
    let raw_errors = data
        .genai_service
        .query_service
        .get_error_breakdown(
            body.service_name.as_deref(),
            body.service_namespace.as_deref(),
            body.service_version.as_deref(),
            body.service_instance_id.as_deref(),
            None,
            body.start_time,
            body.end_time,
            body.operation_name.as_deref(),
            body.agent_name.as_deref(),
            body.model.as_deref(),
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

#[utoipa::path(
    post,
    path = "/scouter/genai/spans",
    request_body = GenAiSpanFilters,
    responses(
        (status = 200, description = "GenAI spans matching the provided filters", body = GenAiSpansResponse),
        (status = 500, description = "Internal server error", body = ScouterServerError),
    ),
    tag = "genai",
    security(("bearer_token" = []))
)]
#[instrument(skip_all)]
pub async fn get_genai_spans(
    State(data): State<Arc<AppState>>,
    Extension(perms): Extension<UserPermissions>,
    Json(body): Json<GenAiSpanFilters>,
) -> Result<Json<GenAiSpansResponse>, (StatusCode, Json<ScouterServerError>)> {
    let include_sensitive = perms.has_permission("read:all");
    let spans = data
        .genai_service
        .query_service
        .get_genai_spans_with_projection(&body, include_sensitive)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::get_genai_spans_error(e)),
            )
        })?;

    Ok(Json(GenAiSpansResponse { spans }))
}

#[utoipa::path(
    get,
    path = "/scouter/genai/conversation/{id}",
    params(
        ("id" = String, Path, description = "Conversation ID"),
        ConversationQuery,
    ),
    responses(
        (status = 200, description = "All spans for the given conversation", body = GenAiSpansResponse),
        (status = 400, description = "Invalid conversation ID or time format", body = ScouterServerError),
        (status = 500, description = "Internal server error", body = ScouterServerError),
    ),
    tag = "genai",
    security(("bearer_token" = []))
)]
#[instrument(skip_all)]
pub async fn get_conversation_spans(
    State(data): State<Arc<AppState>>,
    Extension(perms): Extension<UserPermissions>,
    Path(id): Path<String>,
    Query(params): Query<ConversationQuery>,
) -> Result<Json<GenAiSpansResponse>, (StatusCode, Json<ScouterServerError>)> {
    if id.len() > 256 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ScouterServerError::new(
                "conversation_id exceeds maximum length".to_string(),
            )),
        ));
    }

    let start_time = params
        .start_time
        .as_deref()
        .map(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .map_err(|_| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(ScouterServerError::new(
                            "Invalid start_time: expected RFC3339 format".to_string(),
                        )),
                    )
                })
        })
        .transpose()?;

    let end_time = params
        .end_time
        .as_deref()
        .map(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .map_err(|_| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(ScouterServerError::new(
                            "Invalid end_time: expected RFC3339 format".to_string(),
                        )),
                    )
                })
        })
        .transpose()?;

    let include_sensitive = perms.has_permission("read:all");
    let spans = data
        .genai_service
        .query_service
        .get_conversation_spans_with_projection(&id, start_time, end_time, include_sensitive)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::get_genai_conversation_error(e)),
            )
        })?;

    Ok(Json(GenAiSpansResponse { spans }))
}

/// Handler for retrieving GenAI spans and aggregate metrics for a specific trace ID, with optional time range and span limit.
#[utoipa::path(
    post,
    path = "/scouter/genai/traces/{id}/metrics",
    params(
        ("id" = String, Path, description = "Trace ID (hex-encoded)")
    ),
    request_body = GenAiTraceMetricsRequest,
    responses(
        (status = 200, description = "Trace-scoped GenAI spans and aggregate metrics", body = GenAiTraceMetricsResponse),
        (status = 400, description = "Invalid trace ID or request", body = ScouterServerError),
        (status = 403, description = "Permission denied", body = ScouterServerError),
        (status = 500, description = "Internal server error", body = ScouterServerError),
    ),
    tag = "genai",
    security(("bearer_token" = []))
)]
#[instrument(skip_all)]
pub async fn get_genai_trace_metrics(
    State(data): State<Arc<AppState>>,
    Extension(perms): Extension<UserPermissions>,
    Path(id): Path<String>,
    Json(body): Json<GenAiTraceMetricsRequest>,
) -> Result<Json<GenAiTraceMetricsResponse>, (StatusCode, Json<ScouterServerError>)> {
    let trace_id = TraceId::from_hex(&id).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ScouterServerError::new(format!("Invalid trace_id: {e}"))),
        )
    })?;
    validate_bucket_interval(&body.bucket_interval)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(ScouterServerError::new(e))))?;

    if body.include_sensitive_content && !perms.has_permission("read:all") {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ScouterServerError::permission_denied()),
        ));
    }

    let end_time = body.end_time.unwrap_or_else(Utc::now);
    let start_time = body
        .start_time
        .unwrap_or_else(|| end_time - chrono::Duration::hours(DEFAULT_TRACE_METRICS_WINDOW_HOURS));
    if start_time >= end_time {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ScouterServerError::new(
                "start_time must be earlier than end_time".to_string(),
            )),
        ));
    }
    if end_time - start_time > chrono::Duration::days(MAX_TRACE_METRICS_WINDOW_DAYS) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ScouterServerError::new(format!(
                "Time window exceeds {} days",
                MAX_TRACE_METRICS_WINDOW_DAYS
            ))),
        ));
    }
    let span_limit = body.span_limit.clamp(1, MAX_TRACE_METRICS_SPAN_LIMIT);

    let aggregate_spans = data
        .genai_service
        .query_service
        .get_genai_spans_for_trace_metrics(&trace_id, start_time, end_time)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::get_genai_trace_metrics_error(e)),
            )
        })?;

    let (spans, spans_truncated, sensitive_content_redacted) = if body.include_sensitive_content {
        let payload_spans = data
            .genai_service
            .query_service
            .get_genai_spans_by_trace_id(&trace_id, start_time, end_time, span_limit)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ScouterServerError::get_genai_trace_metrics_error(e)),
                )
            })?;
        (payload_spans.spans, payload_spans.spans_truncated, false)
    } else {
        let mut spans = aggregate_spans.clone();
        let spans_truncated = spans.len() > span_limit;
        if spans_truncated {
            spans.truncate(span_limit);
        }
        (spans, spans_truncated, true)
    };

    let response = build_trace_metrics_response(
        id,
        &aggregate_spans,
        spans,
        &body.bucket_interval,
        &body.model_pricing,
        span_limit,
        spans_truncated,
        sensitive_content_redacted,
    )
    .map_err(|e| (StatusCode::BAD_REQUEST, Json(ScouterServerError::new(e))))?;

    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/scouter/genai/agent/metrics",
    request_body = AgentDashboardRequest,
    responses(
        (status = 200, description = "Agent dashboard time-series and summary", body = AgentDashboardResponse),
        (status = 500, description = "Internal server error", body = ScouterServerError),
    ),
    tag = "genai",
    security(("bearer_token" = []))
)]
#[instrument(skip_all)]
pub async fn get_agent_dashboard(
    State(data): State<Arc<AppState>>,
    Json(body): Json<AgentDashboardRequest>,
) -> Result<Json<AgentDashboardResponse>, (StatusCode, Json<ScouterServerError>)> {
    for (field_name, val) in [
        ("entity_id", body.entity_id.as_deref()),
        ("service_name", body.service_name.as_deref()),
        ("agent_name", body.agent_name.as_deref()),
        ("provider_name", body.provider_name.as_deref()),
    ] {
        if val.is_some_and(|v| v.len() > 256) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ScouterServerError::new(format!(
                    "{field_name} exceeds maximum length of 256 characters"
                ))),
            ));
        }
    }

    validate_bucket_interval(&body.bucket_interval)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(ScouterServerError::new(e))))?;
    if body.start_time >= body.end_time {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ScouterServerError::new(
                "start_time must be earlier than end_time".to_string(),
            )),
        ));
    }
    if body.end_time - body.start_time > chrono::Duration::days(MAX_DASHBOARD_WINDOW_DAYS) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ScouterServerError::new(format!(
                "Time window exceeds {MAX_DASHBOARD_WINDOW_DAYS} days"
            ))),
        ));
    }

    let q = &data.genai_service.query_service;
    let (response, _) = build_agent_panel(
        q,
        body.service_name.as_deref(),
        body.service_namespace.as_deref(),
        body.service_version.as_deref(),
        body.service_instance_id.as_deref(),
        body.entity_id.as_deref(),
        body.start_time,
        body.end_time,
        &body.bucket_interval,
        body.agent_name.as_deref(),
        body.provider_name.as_deref(),
        None,
        &body.model_pricing,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ScouterServerError::get_agent_dashboard_error(e)),
        )
    })?;

    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/scouter/genai/tool/metrics",
    request_body = ToolDashboardRequest,
    responses(
        (status = 200, description = "Tool call aggregates and time-series", body = ToolDashboardResponse),
        (status = 500, description = "Internal server error", body = ScouterServerError),
    ),
    tag = "genai",
    security(("bearer_token" = []))
)]
#[instrument(skip_all)]
pub async fn get_tool_dashboard(
    State(data): State<Arc<AppState>>,
    Json(body): Json<ToolDashboardRequest>,
) -> Result<Json<ToolDashboardResponse>, (StatusCode, Json<ScouterServerError>)> {
    for (field_name, val) in [
        ("service_name", body.service_name.as_deref()),
        ("agent_name", body.agent_name.as_deref()),
        ("provider_name", body.provider_name.as_deref()),
        ("model", body.model.as_deref()),
    ] {
        if val.is_some_and(|v| v.len() > 256) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ScouterServerError::new(format!(
                    "{field_name} exceeds maximum length of 256 characters"
                ))),
            ));
        }
    }

    validate_bucket_interval(&body.bucket_interval)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(ScouterServerError::new(e))))?;
    if body.start_time >= body.end_time {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ScouterServerError::new(
                "start_time must be earlier than end_time".to_string(),
            )),
        ));
    }
    if body.end_time - body.start_time > chrono::Duration::days(MAX_DASHBOARD_WINDOW_DAYS) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ScouterServerError::new(format!(
                "Time window exceeds {MAX_DASHBOARD_WINDOW_DAYS} days"
            ))),
        ));
    }

    let q = &data.genai_service.query_service;
    let (tool_dashboard, _) = build_tool_panel(
        q,
        body.service_name.as_deref(),
        body.service_namespace.as_deref(),
        body.service_version.as_deref(),
        body.service_instance_id.as_deref(),
        None,
        body.start_time,
        body.end_time,
        &body.bucket_interval,
        body.agent_name.as_deref(),
        body.provider_name.as_deref(),
        body.model.as_deref(),
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ScouterServerError::get_tool_dashboard_error(e)),
        )
    })?;

    Ok(Json(tool_dashboard))
}

#[utoipa::path(
    post,
    path = "/scouter/genai/dashboard",
    request_body = GenAiDashboardRequest,
    responses(
        (status = 200, description = "Composite GenAI dashboard with applied/available filters and metadata", body = GenAiDashboardResponse),
        (status = 400, description = "Invalid request (bucket_interval, time window)", body = ScouterServerError),
        (status = 500, description = "Internal server error", body = ScouterServerError),
    ),
    tag = "genai",
    security(("bearer_token" = []))
)]
#[instrument(skip_all)]
pub async fn get_genai_dashboard(
    State(data): State<Arc<AppState>>,
    Json(body): Json<GenAiDashboardRequest>,
) -> Result<Json<GenAiDashboardResponse>, (StatusCode, Json<ScouterServerError>)> {
    for (field_name, val) in [
        ("entity_id", body.entity_id.as_deref()),
        ("service_name", body.service_name.as_deref()),
        ("agent_name", body.agent_name.as_deref()),
        ("provider_name", body.provider_name.as_deref()),
        ("operation_name", body.operation_name.as_deref()),
        ("model", body.model.as_deref()),
    ] {
        if val.is_some_and(|v| v.len() > 256) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ScouterServerError::new(format!(
                    "{field_name} exceeds maximum length of 256 characters"
                ))),
            ));
        }
    }

    if body.model_pricing.len() > 500 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ScouterServerError::new(
                "model_pricing exceeds maximum of 500 entries".to_string(),
            )),
        ));
    }
    if body.model_pricing.keys().any(|k| k.len() > 256) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ScouterServerError::new(
                "model_pricing key exceeds maximum length of 256 characters".to_string(),
            )),
        ));
    }

    validate_bucket_interval(&body.bucket_interval)
        .map_err(|e| (StatusCode::BAD_REQUEST, Json(ScouterServerError::new(e))))?;
    if body.start_time >= body.end_time {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ScouterServerError::new(
                "start_time must be earlier than end_time".to_string(),
            )),
        ));
    }
    if body.end_time - body.start_time > chrono::Duration::days(MAX_DASHBOARD_WINDOW_DAYS) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ScouterServerError::new(format!(
                "Time window exceeds {MAX_DASHBOARD_WINDOW_DAYS} days"
            ))),
        ));
    }

    let q = &data.genai_service.query_service;
    let svc = body.service_name.as_deref();
    let ns = body.service_namespace.as_deref();
    let ver = body.service_version.as_deref();
    let iid = body.service_instance_id.as_deref();
    let eid = body.entity_id.as_deref();
    let agent = body.agent_name.as_deref();
    let provider = body.provider_name.as_deref();
    let operation = body.operation_name.as_deref();
    let model = body.model.as_deref();

    let (
        token_buckets,
        operations_breakdown,
        models_usage,
        (agent_dashboard, agent_truncated),
        (tool_dashboard, tool_truncated),
        error_rows,
        agents_list,
        distinct_filters,
        total_spans,
    ) = tokio::try_join!(
        q.get_token_metrics(
            svc,
            ns,
            ver,
            iid,
            eid,
            body.start_time,
            body.end_time,
            &body.bucket_interval,
            operation,
            provider,
            agent,
            model
        ),
        q.get_operation_breakdown(
            svc,
            ns,
            ver,
            iid,
            eid,
            body.start_time,
            body.end_time,
            provider,
            agent,
            model
        ),
        q.get_model_usage(
            svc,
            ns,
            ver,
            iid,
            eid,
            body.start_time,
            body.end_time,
            provider,
            agent,
            model
        ),
        build_agent_panel(
            q,
            svc,
            ns,
            ver,
            iid,
            eid,
            body.start_time,
            body.end_time,
            &body.bucket_interval,
            agent,
            provider,
            model,
            &body.model_pricing,
        ),
        build_tool_panel(
            q,
            svc,
            ns,
            ver,
            iid,
            eid,
            body.start_time,
            body.end_time,
            &body.bucket_interval,
            agent,
            provider,
            model,
        ),
        q.get_error_breakdown(
            svc,
            ns,
            ver,
            iid,
            eid,
            body.start_time,
            body.end_time,
            operation,
            agent,
            model
        ),
        q.get_agent_activity(
            svc,
            ns,
            ver,
            iid,
            eid,
            body.start_time,
            body.end_time,
            None,
            provider
        ),
        q.get_distinct_filter_values(svc, eid, body.start_time, body.end_time),
        q.count_total_spans(
            svc,
            eid,
            body.start_time,
            body.end_time,
            agent,
            provider,
            operation,
            model
        ),
    )
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ScouterServerError::get_genai_dashboard_error(e)),
        )
    })?;

    let response = build_genai_dashboard_response(
        body,
        token_buckets,
        operations_breakdown,
        models_usage,
        agent_dashboard,
        agent_truncated,
        tool_dashboard,
        tool_truncated,
        error_rows,
        agents_list,
        distinct_filters,
        total_spans,
    );

    Ok(Json(response))
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
        .route(
            &format!("{prefix}/genai/agent/metrics"),
            post(get_agent_dashboard),
        )
        .route(
            &format!("{prefix}/genai/tool/metrics"),
            post(get_tool_dashboard),
        )
        .route(
            &format!("{prefix}/genai/traces/{{id}}/metrics"),
            post(get_genai_trace_metrics),
        )
        .route(
            &format!("{prefix}/genai/dashboard"),
            post(get_genai_dashboard),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, TimeZone};
    use scouter_types::{GenAiEvalResult, SpanId};
    use std::collections::HashMap;

    fn make_trace_span(
        trace_id: TraceId,
        span_id: u8,
        start_time: DateTime<Utc>,
    ) -> GenAiSpanRecord {
        GenAiSpanRecord {
            trace_id,
            span_id: SpanId::from_bytes([span_id; 8]),
            service_name: "svc".to_string(),
            start_time,
            end_time: Some(start_time + chrono::Duration::milliseconds(100)),
            duration_ms: 100,
            status_code: 0,
            operation_name: Some("invoke_agent".to_string()),
            provider_name: Some("openai".to_string()),
            request_model: Some("gpt-4".to_string()),
            response_model: Some("gpt-4o".to_string()),
            input_tokens: Some(10),
            output_tokens: Some(20),
            cache_creation_input_tokens: Some(2),
            cache_read_input_tokens: Some(3),
            conversation_id: Some("conv-1".to_string()),
            agent_name: Some("agent-a".to_string()),
            agent_id: Some("agent-id-a".to_string()),
            tool_name: Some("search".to_string()),
            tool_type: Some("function".to_string()),
            input_messages: Some(r#"[{"role":"user","content":"hi"}]"#.to_string()),
            output_messages: Some(r#"[{"role":"assistant","content":"hello"}]"#.to_string()),
            eval_results: vec![GenAiEvalResult {
                name: "quality".to_string(),
                score_label: Some("pass".to_string()),
                score_value: Some(1.0),
                explanation: Some("ok".to_string()),
                response_id: Some("resp-1".to_string()),
            }],
            ..Default::default()
        }
    }

    #[test]
    fn test_trace_metrics_response_preserves_spans_and_adds_aggregates() {
        let trace_id = TraceId::from_bytes([7u8; 16]);
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 12, 30, 0).unwrap();
        let spans = vec![
            make_trace_span(trace_id, 1, start),
            GenAiSpanRecord {
                error_type: Some("timeout".to_string()),
                duration_ms: 200,
                input_tokens: Some(5),
                output_tokens: Some(15),
                ..make_trace_span(trace_id, 2, start + chrono::Duration::minutes(5))
            },
        ];
        let payload_spans = spans.clone();

        let response = build_trace_metrics_response(
            trace_id.to_hex(),
            &spans,
            payload_spans,
            "hour",
            &HashMap::new(),
            500,
            false,
            false,
        )
        .expect("trace metrics should build");

        assert!(response.has_genai_spans);
        assert_eq!(response.spans.len(), 2);
        assert_eq!(response.span_limit, 500);
        assert!(!response.spans_truncated);
        assert!(!response.sensitive_content_redacted);
        assert_eq!(
            response.spans[0].input_messages.as_deref(),
            Some(r#"[{"role":"user","content":"hi"}]"#)
        );
        assert_eq!(response.spans[0].eval_results.len(), 1);
        assert_eq!(response.token_metrics.buckets.len(), 1);
        assert_eq!(response.token_metrics.buckets[0].total_input_tokens, 15);
        assert_eq!(response.operation_breakdown.operations[0].span_count, 2);
        assert_eq!(response.model_usage.models[0].model, "gpt-4o");
        assert_eq!(response.agent_activity.agents[0].span_count, 2);
        assert_eq!(response.agent_dashboard.summary.total_requests, 2);
        assert_eq!(response.tool_dashboard.aggregates[0].call_count, 2);
        assert_eq!(response.error_breakdown.errors[0].error_type, "timeout");
    }

    #[test]
    fn test_trace_metrics_response_empty_state() {
        let trace_id = TraceId::from_bytes([8u8; 16]);
        let response = build_trace_metrics_response(
            trace_id.to_hex(),
            &[],
            Vec::new(),
            "hour",
            &HashMap::new(),
            500,
            false,
            true,
        )
        .expect("empty trace metrics should build");

        assert!(!response.has_genai_spans);
        assert!(response.spans.is_empty());
        assert!(response.sensitive_content_redacted);
        assert!(response.token_metrics.buckets.is_empty());
        assert!(response.agent_dashboard.buckets.is_empty());
        assert!(response.tool_dashboard.aggregates.is_empty());
    }

    #[test]
    fn test_trace_metrics_response_uses_untruncated_data_for_aggregates() {
        let trace_id = TraceId::from_bytes([9u8; 16]);
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 12, 30, 0).unwrap();
        let aggregate_spans = vec![
            make_trace_span(trace_id, 1, start),
            make_trace_span(trace_id, 2, start + chrono::Duration::minutes(1)),
        ];
        let payload_spans = vec![aggregate_spans[0].clone()];

        let response = build_trace_metrics_response(
            trace_id.to_hex(),
            &aggregate_spans,
            payload_spans,
            "hour",
            &HashMap::new(),
            1,
            true,
            true,
        )
        .expect("trace metrics should build");

        assert_eq!(response.spans.len(), 1);
        assert_eq!(response.operation_breakdown.operations[0].span_count, 2);
        assert_eq!(response.token_metrics.buckets[0].span_count, 2);
    }

    #[test]
    fn test_build_genai_dashboard_response_truncation() {
        use crate::api::routes::trace::metrics::{
            MAX_DASHBOARD_BUCKETS, build_genai_dashboard_response,
        };
        use chrono::Utc;
        use scouter_types::{
            AgentDashboardResponse, DistinctFilterValues, GenAiDashboardRequest, GenAiTokenBucket,
            ToolDashboardResponse,
        };
        use std::collections::HashMap;

        let start = Utc::now() - chrono::Duration::hours(2);
        let end = Utc::now();
        let request = GenAiDashboardRequest {
            service_name: None,
            service_namespace: None,
            service_version: None,
            service_instance_id: None,
            entity_id: None,
            start_time: start,
            end_time: end,
            bucket_interval: "hour".to_string(),
            agent_name: None,
            provider_name: None,
            operation_name: None,
            model: None,
            model_pricing: HashMap::new(),
        };

        // token_buckets over limit triggers truncation
        let token_buckets: Vec<GenAiTokenBucket> = (0..MAX_DASHBOARD_BUCKETS + 1)
            .map(|_| GenAiTokenBucket::default())
            .collect();

        let response = build_genai_dashboard_response(
            request.clone(),
            token_buckets,
            vec![],
            vec![],
            AgentDashboardResponse::default(),
            false,
            ToolDashboardResponse::default(),
            false,
            vec![],
            vec![],
            DistinctFilterValues::default(),
            0,
        );
        assert!(
            response.buckets_truncated,
            "Expected buckets_truncated=true when token_buckets over limit"
        );
        assert_eq!(response.token_metrics.buckets.len(), MAX_DASHBOARD_BUCKETS);

        // all under limit → not truncated
        let response2 = build_genai_dashboard_response(
            request.clone(),
            vec![GenAiTokenBucket::default(); 10],
            vec![],
            vec![],
            AgentDashboardResponse::default(),
            false,
            ToolDashboardResponse::default(),
            false,
            vec![],
            vec![],
            DistinctFilterValues::default(),
            0,
        );
        assert!(
            !response2.buckets_truncated,
            "Expected buckets_truncated=false when all vecs under limit"
        );

        // agent_truncated=true → buckets_truncated
        let response3 = build_genai_dashboard_response(
            request,
            vec![],
            vec![],
            vec![],
            AgentDashboardResponse::default(),
            true,
            ToolDashboardResponse::default(),
            false,
            vec![],
            vec![],
            DistinctFilterValues::default(),
            0,
        );
        assert!(
            response3.buckets_truncated,
            "Expected buckets_truncated=true when agent_truncated=true"
        );
    }
}
