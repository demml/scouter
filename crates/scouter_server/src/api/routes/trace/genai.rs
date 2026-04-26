use crate::api::state::AppState;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use scouter_types::{
    contracts::ScouterServerError, AgentActivityQuery, AgentBucketRow, AgentDashboardRequest,
    AgentDashboardResponse, AgentDashboardSummary, AgentMetricBucket, ConversationQuery,
    GenAiAgentActivityResponse, GenAiErrorBreakdownResponse, GenAiErrorCount, GenAiMetricsRequest,
    GenAiModelUsage, GenAiModelUsageResponse, GenAiOperationBreakdown,
    GenAiOperationBreakdownResponse, GenAiSpanFilters, GenAiSpanRecord, GenAiSpansResponse,
    GenAiTokenBucket, GenAiTokenMetricsResponse, GenAiToolActivity, GenAiToolActivityResponse,
    GenAiTraceMetricsRequest, GenAiTraceMetricsResponse, ModelCostBreakdown, ModelPricing,
    ToolDashboardRequest, ToolDashboardResponse, ToolTimeBucket, TraceId,
};

use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tracing::instrument;

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

const VALID_BUCKET_INTERVALS: &[&str] =
    &["second", "minute", "hour", "day", "week", "month", "year"];

type OperationGroupKey = (String, Option<String>);
type OperationGroupTotals = (i64, i64, i64, i64, i64);
type AgentActivityKey = (Option<String>, Option<String>, Option<String>);
type AgentActivityTotals = (i64, i64, i64, Option<DateTime<Utc>>);
type ToolGroupKey = (Option<String>, Option<String>);
type ToolTimeGroupKey = (DateTime<Utc>, Option<String>, Option<String>);

fn validate_bucket_interval(bucket_interval: &str) -> Result<(), String> {
    if VALID_BUCKET_INTERVALS.contains(&bucket_interval) {
        Ok(())
    } else {
        Err(format!(
            "Invalid bucket_interval '{}'. Must be one of: {}",
            bucket_interval,
            VALID_BUCKET_INTERVALS.join(", ")
        ))
    }
}

fn truncate_to_bucket(
    timestamp: DateTime<Utc>,
    bucket_interval: &str,
) -> Result<DateTime<Utc>, String> {
    let date = timestamp.date_naive();
    let bucket = match bucket_interval {
        "second" => date.and_hms_opt(timestamp.hour(), timestamp.minute(), timestamp.second()),
        "minute" => date.and_hms_opt(timestamp.hour(), timestamp.minute(), 0),
        "hour" => date.and_hms_opt(timestamp.hour(), 0, 0),
        "day" => date.and_hms_opt(0, 0, 0),
        "week" => {
            let week_start =
                date - chrono::Duration::days(timestamp.weekday().num_days_from_monday() as i64);
            week_start.and_hms_opt(0, 0, 0)
        }
        "month" => chrono::NaiveDate::from_ymd_opt(timestamp.year(), timestamp.month(), 1)
            .and_then(|d| d.and_hms_opt(0, 0, 0)),
        "year" => chrono::NaiveDate::from_ymd_opt(timestamp.year(), 1, 1)
            .and_then(|d| d.and_hms_opt(0, 0, 0)),
        _ => return Err(format!("Invalid bucket_interval '{bucket_interval}'")),
    }
    .ok_or_else(|| format!("Failed to truncate timestamp for bucket '{bucket_interval}'"))?;

    Ok(Utc.from_utc_datetime(&bucket))
}

fn percentile(values: &[i64], quantile: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let idx = ((sorted.len() - 1) as f64 * quantile).round() as usize;
    Some(sorted[idx] as f64)
}

fn model_for_span(span: &GenAiSpanRecord) -> String {
    span.response_model
        .clone()
        .or_else(|| span.request_model.clone())
        .unwrap_or_else(|| "unknown".to_string())
}

fn is_agent_span(span: &GenAiSpanRecord) -> bool {
    matches!(
        span.operation_name.as_deref(),
        Some("invoke_agent" | "create_agent")
    ) || span.agent_name.is_some()
}

fn is_tool_span(span: &GenAiSpanRecord) -> bool {
    span.tool_name.is_some() || matches!(span.operation_name.as_deref(), Some("execute_tool"))
}

fn build_token_metrics(
    spans: &[GenAiSpanRecord],
    bucket_interval: &str,
) -> Result<GenAiTokenMetricsResponse, String> {
    let mut buckets: HashMap<DateTime<Utc>, GenAiTokenBucket> = HashMap::new();
    for span in spans {
        let bucket_start = truncate_to_bucket(span.start_time, bucket_interval)?;
        let bucket = buckets
            .entry(bucket_start)
            .or_insert_with(|| GenAiTokenBucket {
                bucket_start,
                ..Default::default()
            });
        bucket.total_input_tokens += span.input_tokens.unwrap_or_default();
        bucket.total_output_tokens += span.output_tokens.unwrap_or_default();
        bucket.total_cache_creation_tokens += span.cache_creation_input_tokens.unwrap_or_default();
        bucket.total_cache_read_tokens += span.cache_read_input_tokens.unwrap_or_default();
        bucket.span_count += 1;
        if span.error_type.is_some() {
            bucket.error_rate += 1.0;
        }
    }

    let mut buckets: Vec<_> = buckets
        .into_values()
        .map(|mut bucket| {
            if bucket.span_count > 0 {
                bucket.error_rate /= bucket.span_count as f64;
            }
            bucket
        })
        .collect();
    buckets.sort_by_key(|bucket| bucket.bucket_start);
    Ok(GenAiTokenMetricsResponse { buckets })
}

fn build_operation_breakdown(spans: &[GenAiSpanRecord]) -> GenAiOperationBreakdownResponse {
    let mut groups: HashMap<OperationGroupKey, OperationGroupTotals> = HashMap::new();
    for span in spans {
        let key = (
            span.operation_name.clone().unwrap_or_default(),
            span.provider_name.clone(),
        );
        let entry = groups.entry(key).or_default();
        entry.0 += 1;
        entry.1 += span.duration_ms;
        entry.2 += span.input_tokens.unwrap_or_default();
        entry.3 += span.output_tokens.unwrap_or_default();
        if span.error_type.is_some() {
            entry.4 += 1;
        }
    }

    let mut operations: Vec<_> = groups
        .into_iter()
        .map(
            |((operation_name, provider_name), (count, duration, input, output, errors))| {
                GenAiOperationBreakdown {
                    operation_name,
                    provider_name,
                    span_count: count,
                    avg_duration_ms: if count > 0 {
                        duration as f64 / count as f64
                    } else {
                        0.0
                    },
                    total_input_tokens: input,
                    total_output_tokens: output,
                    error_rate: if count > 0 {
                        errors as f64 / count as f64
                    } else {
                        0.0
                    },
                }
            },
        )
        .collect();
    operations.sort_by_key(|operation| Reverse(operation.span_count));
    GenAiOperationBreakdownResponse { operations }
}

fn build_model_usage(spans: &[GenAiSpanRecord]) -> GenAiModelUsageResponse {
    let mut groups: HashMap<(String, Option<String>), Vec<&GenAiSpanRecord>> = HashMap::new();
    for span in spans {
        groups
            .entry((model_for_span(span), span.provider_name.clone()))
            .or_default()
            .push(span);
    }

    let mut models: Vec<_> = groups
        .into_iter()
        .map(|((model, provider_name), rows)| {
            let durations: Vec<i64> = rows.iter().map(|span| span.duration_ms).collect();
            let span_count = rows.len() as i64;
            let errors = rows.iter().filter(|span| span.error_type.is_some()).count() as i64;
            GenAiModelUsage {
                model,
                provider_name,
                span_count,
                total_input_tokens: rows
                    .iter()
                    .map(|span| span.input_tokens.unwrap_or_default())
                    .sum(),
                total_output_tokens: rows
                    .iter()
                    .map(|span| span.output_tokens.unwrap_or_default())
                    .sum(),
                p50_duration_ms: percentile(&durations, 0.5),
                p95_duration_ms: percentile(&durations, 0.95),
                error_rate: if span_count > 0 {
                    errors as f64 / span_count as f64
                } else {
                    0.0
                },
            }
        })
        .collect();
    models.sort_by_key(|model| Reverse(model.span_count));
    GenAiModelUsageResponse { models }
}

fn build_agent_activity(spans: &[GenAiSpanRecord]) -> GenAiAgentActivityResponse {
    let mut groups: HashMap<AgentActivityKey, AgentActivityTotals> = HashMap::new();

    for span in spans.iter().filter(|span| is_agent_span(span)) {
        let key = (
            span.agent_name.clone(),
            span.agent_id.clone(),
            span.conversation_id.clone(),
        );
        let entry = groups.entry(key).or_default();
        entry.0 += 1;
        entry.1 += span.input_tokens.unwrap_or_default();
        entry.2 += span.output_tokens.unwrap_or_default();
        entry.3 = Some(
            entry
                .3
                .map_or(span.start_time, |last| last.max(span.start_time)),
        );
    }

    let agents = groups
        .into_iter()
        .map(
            |((agent_name, agent_id, conversation_id), (span_count, input, output, last_seen))| {
                scouter_types::GenAiAgentActivity {
                    agent_name,
                    agent_id,
                    conversation_id,
                    span_count,
                    total_input_tokens: input,
                    total_output_tokens: output,
                    last_seen,
                }
            },
        )
        .collect();

    GenAiAgentActivityResponse { agents }
}

fn build_agent_dashboard(
    spans: &[GenAiSpanRecord],
    bucket_interval: &str,
    model_pricing: &std::collections::HashMap<String, ModelPricing>,
) -> Result<AgentDashboardResponse, String> {
    let agent_spans: Vec<_> = spans.iter().filter(|span| is_agent_span(span)).collect();
    let mut groups: HashMap<(DateTime<Utc>, String), Vec<&GenAiSpanRecord>> = HashMap::new();
    let mut unique_agents = HashSet::new();
    let mut unique_conversations = HashSet::new();

    for span in agent_spans {
        if let Some(agent_name) = &span.agent_name {
            unique_agents.insert(agent_name.clone());
        }
        if let Some(conversation_id) = &span.conversation_id {
            unique_conversations.insert(conversation_id.clone());
        }
        groups
            .entry((
                truncate_to_bucket(span.start_time, bucket_interval)?,
                model_for_span(span),
            ))
            .or_default()
            .push(span);
    }

    let rows: Vec<_> = groups
        .into_iter()
        .map(|((bucket_start, model), rows)| {
            let span_count = rows.len() as i64;
            let error_count = rows.iter().filter(|span| span.error_type.is_some()).count() as i64;
            let durations: Vec<i64> = rows.iter().map(|span| span.duration_ms).collect();
            AgentBucketRow {
                bucket_start,
                model: Some(model),
                span_count,
                error_count,
                error_rate: if span_count > 0 {
                    error_count as f64 / span_count as f64
                } else {
                    0.0
                },
                avg_duration_ms: if span_count > 0 {
                    durations.iter().sum::<i64>() as f64 / span_count as f64
                } else {
                    0.0
                },
                p50_duration_ms: percentile(&durations, 0.5),
                p95_duration_ms: percentile(&durations, 0.95),
                p99_duration_ms: percentile(&durations, 0.99),
                input_tokens: rows
                    .iter()
                    .map(|span| span.input_tokens.unwrap_or_default())
                    .sum(),
                output_tokens: rows
                    .iter()
                    .map(|span| span.output_tokens.unwrap_or_default())
                    .sum(),
                cache_creation_tokens: rows
                    .iter()
                    .map(|span| span.cache_creation_input_tokens.unwrap_or_default())
                    .sum(),
                cache_read_tokens: rows
                    .iter()
                    .map(|span| span.cache_read_input_tokens.unwrap_or_default())
                    .sum(),
            }
        })
        .collect();

    let mut response = fold_agent_buckets(&rows, model_pricing);
    response.summary.unique_agent_count = unique_agents.len() as i64;
    response.summary.unique_conversation_count = unique_conversations.len() as i64;
    Ok(response)
}

fn build_tool_dashboard(
    spans: &[GenAiSpanRecord],
    bucket_interval: &str,
) -> Result<ToolDashboardResponse, String> {
    let tool_spans: Vec<_> = spans.iter().filter(|span| is_tool_span(span)).collect();
    let mut aggregate_groups: HashMap<ToolGroupKey, Vec<&GenAiSpanRecord>> = HashMap::new();
    let mut time_groups: HashMap<ToolTimeGroupKey, Vec<&GenAiSpanRecord>> = HashMap::new();

    for span in tool_spans {
        let key = (span.tool_name.clone(), span.tool_type.clone());
        aggregate_groups.entry(key.clone()).or_default().push(span);
        time_groups
            .entry((
                truncate_to_bucket(span.start_time, bucket_interval)?,
                key.0,
                key.1,
            ))
            .or_default()
            .push(span);
    }

    let aggregates = aggregate_groups
        .into_iter()
        .map(|((tool_name, tool_type), rows)| {
            let call_count = rows.len() as i64;
            let error_count = rows.iter().filter(|span| span.error_type.is_some()).count() as i64;
            GenAiToolActivity {
                tool_name,
                tool_type,
                call_count,
                avg_duration_ms: if call_count > 0 {
                    rows.iter().map(|span| span.duration_ms).sum::<i64>() as f64 / call_count as f64
                } else {
                    0.0
                },
                error_rate: if call_count > 0 {
                    error_count as f64 / call_count as f64
                } else {
                    0.0
                },
            }
        })
        .collect();

    let mut time_series: Vec<_> = time_groups
        .into_iter()
        .map(|((bucket_start, tool_name, tool_type), rows)| {
            let call_count = rows.len() as i64;
            let error_count = rows.iter().filter(|span| span.error_type.is_some()).count() as i64;
            ToolTimeBucket {
                bucket_start,
                tool_name,
                tool_type,
                call_count,
                avg_duration_ms: if call_count > 0 {
                    rows.iter().map(|span| span.duration_ms).sum::<i64>() as f64 / call_count as f64
                } else {
                    0.0
                },
                error_rate: if call_count > 0 {
                    error_count as f64 / call_count as f64
                } else {
                    0.0
                },
            }
        })
        .collect();
    time_series.sort_by_key(|bucket| bucket.bucket_start);

    Ok(ToolDashboardResponse {
        aggregates,
        time_series,
    })
}

fn build_error_breakdown(spans: &[GenAiSpanRecord]) -> GenAiErrorBreakdownResponse {
    let mut groups: HashMap<String, i64> = HashMap::new();
    for error_type in spans.iter().filter_map(|span| span.error_type.as_ref()) {
        *groups.entry(error_type.clone()).or_default() += 1;
    }
    let mut errors: Vec<_> = groups
        .into_iter()
        .map(|(error_type, count)| GenAiErrorCount { error_type, count })
        .collect();
    errors.sort_by_key(|error| Reverse(error.count));
    GenAiErrorBreakdownResponse { errors }
}

fn build_trace_metrics_response(
    trace_id: String,
    spans: Vec<GenAiSpanRecord>,
    bucket_interval: &str,
    model_pricing: &std::collections::HashMap<String, ModelPricing>,
) -> Result<GenAiTraceMetricsResponse, String> {
    validate_bucket_interval(bucket_interval)?;
    let token_metrics = build_token_metrics(&spans, bucket_interval)?;
    let agent_dashboard = build_agent_dashboard(&spans, bucket_interval, model_pricing)?;
    let tool_dashboard = build_tool_dashboard(&spans, bucket_interval)?;

    Ok(GenAiTraceMetricsResponse {
        trace_id,
        has_genai_spans: !spans.is_empty(),
        operation_breakdown: build_operation_breakdown(&spans),
        model_usage: build_model_usage(&spans),
        agent_activity: build_agent_activity(&spans),
        error_breakdown: build_error_breakdown(&spans),
        spans,
        token_metrics,
        agent_dashboard,
        tool_dashboard,
    })
}

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
        (status = 500, description = "Internal server error", body = ScouterServerError),
    ),
    tag = "genai",
    security(("bearer_token" = []))
)]
#[instrument(skip_all)]
pub async fn get_genai_trace_metrics(
    State(data): State<Arc<AppState>>,
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

    let spans = data
        .genai_service
        .query_service
        .get_genai_spans_by_trace_id(&trace_id, body.start_time, body.end_time)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::get_genai_trace_metrics_error(e)),
            )
        })?;

    let response =
        build_trace_metrics_response(id, spans, &body.bucket_interval, &body.model_pricing)
            .map_err(|e| (StatusCode::BAD_REQUEST, Json(ScouterServerError::new(e))))?;

    Ok(Json(response))
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
        .route(
            &format!("{prefix}/genai/traces/{{id}}/metrics"),
            post(get_genai_trace_metrics),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use scouter_types::{GenAiEvalResult, SpanId};

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

        let response =
            build_trace_metrics_response(trace_id.to_hex(), spans, "hour", &HashMap::new())
                .expect("trace metrics should build");

        assert!(response.has_genai_spans);
        assert_eq!(response.spans.len(), 2);
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
        let response =
            build_trace_metrics_response(trace_id.to_hex(), Vec::new(), "hour", &HashMap::new())
                .expect("empty trace metrics should build");

        assert!(!response.has_genai_spans);
        assert!(response.spans.is_empty());
        assert!(response.token_metrics.buckets.is_empty());
        assert!(response.agent_dashboard.buckets.is_empty());
        assert!(response.tool_dashboard.aggregates.is_empty());
    }
}
