use crate::api::state::AppState;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use scouter_types::{
    contracts::ScouterServerError, AgentBucketRow, AgentDashboardRequest, AgentDashboardResponse,
    AgentDashboardSummary, AgentMetricBucket, GenAiAgentActivityResponse,
    GenAiErrorBreakdownResponse, GenAiErrorCount, GenAiMetricsRequest, GenAiModelUsageResponse,
    GenAiOperationBreakdownResponse, GenAiSpanFilters, GenAiSpansResponse,
    GenAiTokenMetricsResponse, GenAiToolActivityResponse, ModelCostBreakdown, ModelPricing,
    ToolDashboardRequest, ToolDashboardResponse,
};
use serde::Deserialize;
use std::sync::Arc;
use tracing::instrument;

#[derive(Deserialize, utoipa::IntoParams)]
pub struct AgentActivityQuery {
    pub agent_name: Option<String>,
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct ConversationQuery {
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

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
    let tools = data
        .genai_service
        .query_service
        .get_tool_activity(body.service_name.as_deref(), body.start_time, body.end_time)
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

fn compute_cost(
    input: i64,
    output: i64,
    cache_creation: i64,
    cache_read: i64,
    pricing: &ModelPricing,
) -> f64 {
    (input as f64 / 1_000_000.0) * pricing.input_per_million
        + (output as f64 / 1_000_000.0) * pricing.output_per_million
        + (cache_creation as f64 / 1_000_000.0) * pricing.cache_creation_per_million
        + (cache_read as f64 / 1_000_000.0) * pricing.cache_read_per_million
}

fn fold_agent_buckets(
    rows: &[AgentBucketRow],
    model_pricing: &std::collections::HashMap<String, ModelPricing>,
) -> AgentDashboardResponse {
    use std::collections::HashMap;

    // Group rows by bucket_start to build time-series buckets.
    // Within each bucket, sum tokens across models and compute weighted latency.
    let mut bucket_map: HashMap<i64, AgentMetricBucket> = HashMap::new();

    // Per-model token accumulator (across all buckets) for the summary.
    let mut model_tokens: HashMap<String, (i64, i64, i64, i64)> = HashMap::new();

    let has_pricing = !model_pricing.is_empty();

    for row in rows {
        let ts = row.bucket_start.timestamp_micros();
        let bucket = bucket_map.entry(ts).or_insert_with(|| AgentMetricBucket {
            bucket_start: row.bucket_start,
            ..Default::default()
        });

        // Aggregate per-bucket totals across models.
        // Weighted avg for latency: accumulate sum_duration = avg * span_count then re-divide.
        let prev_count = bucket.span_count;
        let new_count = prev_count + row.span_count;
        if new_count > 0 {
            bucket.avg_duration_ms = (bucket.avg_duration_ms * prev_count as f64
                + row.avg_duration_ms * row.span_count as f64)
                / new_count as f64;
        }
        bucket.span_count = new_count;
        bucket.error_count += row.error_count;
        if bucket.span_count > 0 {
            bucket.error_rate = bucket.error_count as f64 / bucket.span_count as f64;
        }
        bucket.total_input_tokens += row.input_tokens;
        bucket.total_output_tokens += row.output_tokens;
        bucket.total_cache_creation_tokens += row.cache_creation_tokens;
        bucket.total_cache_read_tokens += row.cache_read_tokens;

        // Percentiles: take the non-null value if bucket doesn't have one yet.
        // For true accuracy a separate percentile query would be needed; this is a best-effort
        // approximation that uses the first model's percentile per bucket.
        if bucket.p50_duration_ms.is_none() {
            bucket.p50_duration_ms = row.p50_duration_ms;
        }
        if bucket.p95_duration_ms.is_none() {
            bucket.p95_duration_ms = row.p95_duration_ms;
        }
        if bucket.p99_duration_ms.is_none() {
            bucket.p99_duration_ms = row.p99_duration_ms;
        }

        // Cost per bucket.
        if has_pricing {
            let model_key = row.model.as_deref().unwrap_or("unknown");
            if let Some(pricing) = model_pricing.get(model_key) {
                let cost = compute_cost(
                    row.input_tokens,
                    row.output_tokens,
                    row.cache_creation_tokens,
                    row.cache_read_tokens,
                    pricing,
                );
                *bucket.total_cost.get_or_insert(0.0) += cost;
            }
        }

        // Accumulate per-model totals for summary.
        let model_key = row.model.clone().unwrap_or_else(|| "unknown".to_string());
        let entry = model_tokens.entry(model_key).or_default();
        entry.0 += row.input_tokens;
        entry.1 += row.output_tokens;
        entry.2 += row.cache_creation_tokens;
        entry.3 += row.cache_read_tokens;
    }

    // Sort buckets by time.
    let mut buckets: Vec<AgentMetricBucket> = bucket_map.into_values().collect();
    buckets.sort_by_key(|b| b.bucket_start);

    // Build summary from all rows.
    let total_requests: i64 = rows.iter().map(|r| r.span_count).sum();
    let total_errors: i64 = rows.iter().map(|r| r.error_count).sum();
    let overall_error_rate = if total_requests > 0 {
        total_errors as f64 / total_requests as f64
    } else {
        0.0
    };
    let avg_duration_ms = if total_requests > 0 {
        rows.iter()
            .map(|r| r.avg_duration_ms * r.span_count as f64)
            .sum::<f64>()
            / total_requests as f64
    } else {
        0.0
    };
    let total_input: i64 = rows.iter().map(|r| r.input_tokens).sum();
    let total_output: i64 = rows.iter().map(|r| r.output_tokens).sum();
    let total_cache_creation: i64 = rows.iter().map(|r| r.cache_creation_tokens).sum();
    let total_cache_read: i64 = rows.iter().map(|r| r.cache_read_tokens).sum();

    // Best-effort global percentiles from first non-null row.
    let p50 = rows.iter().find_map(|r| r.p50_duration_ms);
    let p95 = rows.iter().find_map(|r| r.p95_duration_ms);
    let p99 = rows.iter().find_map(|r| r.p99_duration_ms);

    let cost_by_model: Vec<ModelCostBreakdown> = model_tokens
        .into_iter()
        .map(|(model, (inp, out, cc, cr))| {
            let total_cost = if has_pricing {
                model_pricing
                    .get(&model)
                    .map(|p| compute_cost(inp, out, cc, cr, p))
            } else {
                None
            };
            ModelCostBreakdown {
                model,
                total_input_tokens: inp,
                total_output_tokens: out,
                total_cache_creation_tokens: cc,
                total_cache_read_tokens: cr,
                total_cost,
            }
        })
        .collect();

    let summary = AgentDashboardSummary {
        total_requests,
        avg_duration_ms,
        p50_duration_ms: p50,
        p95_duration_ms: p95,
        p99_duration_ms: p99,
        overall_error_rate,
        total_input_tokens: total_input,
        total_output_tokens: total_output,
        total_cache_creation_tokens: total_cache_creation,
        total_cache_read_tokens: total_cache_read,
        unique_agent_count: 0, // filled by caller after get_agent_unique_counts
        unique_conversation_count: 0,
        cost_by_model,
    };

    AgentDashboardResponse { summary, buckets }
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
    let rows = data
        .genai_service
        .query_service
        .get_agent_metrics_by_bucket(
            body.service_name.as_deref(),
            body.start_time,
            body.end_time,
            &body.bucket_interval,
            body.agent_name.as_deref(),
            body.provider_name.as_deref(),
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::get_agent_dashboard_error(e)),
            )
        })?;

    let (unique_agent_count, unique_conversation_count) = data
        .genai_service
        .query_service
        .get_agent_unique_counts(
            body.service_name.as_deref(),
            body.start_time,
            body.end_time,
            body.agent_name.as_deref(),
            body.provider_name.as_deref(),
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::get_agent_dashboard_error(e)),
            )
        })?;

    let mut response = fold_agent_buckets(&rows, &body.model_pricing);
    response.summary.unique_agent_count = unique_agent_count;
    response.summary.unique_conversation_count = unique_conversation_count;

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
    let (aggregates, time_series) = tokio::try_join!(
        data.genai_service.query_service.get_tool_activity(
            body.service_name.as_deref(),
            body.start_time,
            body.end_time
        ),
        data.genai_service
            .query_service
            .get_tool_metrics_timeseries(
                body.service_name.as_deref(),
                body.start_time,
                body.end_time,
                &body.bucket_interval,
            ),
    )
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ScouterServerError::get_tool_dashboard_error(e)),
        )
    })?;

    Ok(Json(ToolDashboardResponse {
        aggregates,
        time_series,
    }))
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
}
