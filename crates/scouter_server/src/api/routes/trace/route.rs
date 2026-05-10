use crate::api::state::AppState;

use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::{Extension, Query, State},
    http::StatusCode,
    routing::{get, post},
};

use axum::body::Bytes;
use axum::extract::Path;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use opentelemetry_proto::tonic::collector::trace::v1::{
    ExportTraceServiceRequest, ExportTraceServiceResponse,
};
use prost::Message;
use scouter_auth::permission::UserPermissions;
use scouter_sql::PostgresClient;
use scouter_sql::sql::traits::{TagSqlLogic, TraceSqlLogic};
use scouter_types::{
    SpansFromTagsRequest, Tag, TraceBaggageResponse, TraceFacetsResponse, TraceId,
    TraceMetricsRequest, TraceMetricsResponse, TracePaginationResponse, TraceRequest,
    TraceServerRecord, TraceSpansResponse,
    contracts::ScouterServerError,
    redact_sensitive_gen_ai_trace_span,
    sql::{TraceFilters, TraceSpan},
    trace::query::{FilterClause, parse_search_query, validate_filter_clause},
};
use std::collections::HashSet;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use tracing::instrument;
use tracing::{debug, error};

#[derive(Debug, serde::Deserialize)]
pub struct SearchQ {
    pub q: Option<String>,
}

fn invalid_search_query(err: impl std::fmt::Display) -> (StatusCode, Json<ScouterServerError>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ScouterServerError::new(format!(
            "Invalid search query: {err}"
        ))),
    )
}

fn trace_query_status(
    err: &scouter_dataframe::error::TraceEngineError,
) -> (StatusCode, Option<String>) {
    match err {
        scouter_dataframe::error::TraceEngineError::IntermediateSetTooLarge { actual, limit } => (
            StatusCode::BAD_REQUEST,
            Some(format!(
                "filter resolved to {actual} traces, exceeds cap of {limit}; narrow the time window or refine the query"
            )),
        ),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, None),
    }
}

fn redact_trace_spans_for_permissions(
    mut spans: Vec<TraceSpan>,
    perms: &UserPermissions,
) -> Vec<TraceSpan> {
    if perms.has_permission("read:all") {
        return spans;
    }
    for span in &mut spans {
        redact_sensitive_gen_ai_trace_span(span);
    }
    spans
}

fn validate_filters(filters: &TraceFilters) -> Result<(), (StatusCode, Json<ScouterServerError>)> {
    if let Some(ref d) = filters.direction
        && d != "next"
        && d != "previous"
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ScouterServerError::new(format!("invalid direction: {d}"))),
        ));
    }
    filters.parsed_trace_ids().map_err(|err| {
        (
            StatusCode::BAD_REQUEST,
            Json(ScouterServerError::new(format!("invalid trace_ids: {err}"))),
        )
    })?;
    filters.parsed_cursor_trace_id().map_err(|err| {
        (
            StatusCode::BAD_REQUEST,
            Json(ScouterServerError::new(format!(
                "invalid cursor_trace_id: {err}"
            ))),
        )
    })?;
    if let Some(clause) = &filters.clause {
        validate_clause(clause)
            .map_err(|msg| (StatusCode::BAD_REQUEST, Json(ScouterServerError::new(msg))))?;
    }
    Ok(())
}

fn validate_clause(clause: &FilterClause) -> Result<(), String> {
    validate_filter_clause(clause).map_err(|err| err.to_string())
}

fn merge_q_into_filters(
    q: Option<String>,
    body: TraceFilters,
) -> Result<TraceFilters, (StatusCode, Json<ScouterServerError>)> {
    let Some(q) = q.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(body);
    };
    let parsed = parse_search_query(q).map_err(invalid_search_query)?;
    Ok(merge_filters(parsed, body))
}

fn merge_filters(parsed: TraceFilters, body: TraceFilters) -> TraceFilters {
    let merge_vec = |parsed: Option<Vec<String>>, body: Option<Vec<String>>| match (parsed, body) {
        (None, None) => None,
        (Some(values), None) | (None, Some(values)) => Some(values),
        (Some(mut parsed), Some(body)) => {
            let mut seen = parsed.iter().cloned().collect::<HashSet<_>>();
            for value in body {
                if seen.insert(value.clone()) {
                    parsed.push(value);
                }
            }
            Some(parsed)
        }
    };

    TraceFilters {
        clause: FilterClause::and_merge(parsed.clause, body.clause),
        start_time: body.start_time.or(parsed.start_time),
        end_time: body.end_time.or(parsed.end_time),
        limit: body.limit.or(parsed.limit),
        cursor_start_time: body.cursor_start_time.or(parsed.cursor_start_time),
        cursor_trace_id: body.cursor_trace_id.or(parsed.cursor_trace_id),
        direction: body.direction.or(parsed.direction),
        trace_ids: merge_vec(parsed.trace_ids, body.trace_ids),
        entity_uid: body.entity_uid.or(parsed.entity_uid),
    }
}

fn normalize_metrics_request(
    mut body: TraceMetricsRequest,
) -> Result<TraceMetricsRequest, (StatusCode, Json<ScouterServerError>)> {
    let Some(query) = body.query.take() else {
        return Ok(body);
    };
    let query = query.trim();
    if query.is_empty() {
        return Ok(body);
    }
    let parsed = parse_search_query(query).map_err(invalid_search_query)?;
    body.clause = FilterClause::and_merge(parsed.clause, body.clause);
    body.entity_uid = body.entity_uid.or(parsed.entity_uid);
    Ok(body)
}

#[utoipa::path(
    get,
    path = "/scouter/trace/baggage",
    params(TraceRequest),
    responses(
        (status = 200, description = "Trace baggage records", body = TraceBaggageResponse),
        (status = 500, description = "Internal server error", body = ScouterServerError),
    ),
    tag = "traces",
    security(("bearer_token" = []))
)]
pub async fn get_trace_baggage(
    State(data): State<Arc<AppState>>,
    Query(params): Query<TraceRequest>,
) -> Result<Json<TraceBaggageResponse>, (StatusCode, Json<ScouterServerError>)> {
    let baggage = PostgresClient::get_trace_baggage_records(&data.db_pool, &params.trace_id)
        .await
        .map_err(|e| {
            error!("Failed to get trace baggage records: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::get_baggage_error(e)),
            )
        })?;

    Ok(Json(TraceBaggageResponse { baggage }))
}

#[utoipa::path(
    post,
    path = "/scouter/trace/paginated",
    request_body = TraceFilters,
    params(("q" = Option<String>, Query, description = "Optional Lucene-style search query")),
    responses(
        (status = 200, description = "Paginated traces", body = TracePaginationResponse),
        (status = 500, description = "Internal server error", body = ScouterServerError),
    ),
    tag = "traces",
    security(("bearer_token" = []))
)]
#[instrument(skip_all)]
pub async fn paginated_traces(
    State(data): State<Arc<AppState>>,
    Query(search): Query<SearchQ>,
    Json(body): Json<TraceFilters>,
) -> Result<Json<TracePaginationResponse>, (StatusCode, Json<ScouterServerError>)> {
    let body = merge_q_into_filters(search.q, body)?;
    validate_filters(&body)?;
    debug!(
        "paginated_traces: limit={:?} start={:?} end={:?}",
        body.limit, body.start_time, body.end_time
    );

    // entity_uid is passed directly to the Delta Lake query where it is applied as a
    // column predicate on the `entity_id` column, enabling file-level Z-ORDER skipping.
    let pagination_response = data
        .trace_summary_service
        .query_service
        .get_paginated_traces(&body)
        .await
        .map_err(|e| {
            error!("Failed to get paginated traces: {:?}", e);
            let (status, message) = trace_query_status(&e);
            (
                status,
                Json(match message {
                    Some(message) => ScouterServerError::new(message),
                    None => ScouterServerError::get_paginated_traces_error(e),
                }),
            )
        })?;

    debug!(
        "Number of traces retrieved: {}",
        pagination_response.items.len()
    );

    Ok(Json(pagination_response))
}

#[utoipa::path(
    get,
    path = "/scouter/v1/traces/{id}/spans",
    params(
        ("id" = String, Path, description = "Trace ID (hex-encoded)")
    ),
    responses(
        (status = 200, description = "Trace spans by ID", body = TraceSpansResponse),
        (status = 400, description = "Invalid trace ID", body = ScouterServerError),
        (status = 500, description = "Internal server error", body = ScouterServerError),
    ),
    tag = "traces",
    security(("bearer_token" = []))
)]
#[instrument(skip_all)]
pub async fn get_trace_spans_by_id(
    State(data): State<Arc<AppState>>,
    Extension(perms): Extension<UserPermissions>,
    Path(id): Path<String>,
) -> Result<Json<TraceSpansResponse>, (StatusCode, Json<ScouterServerError>)> {
    debug!("Getting trace spans for trace_id: {}", id);
    let trace_id_bytes = TraceId::hex_to_bytes(&id).map_err(|e| {
        error!("Invalid trace_id hex: {:?}", e);
        (
            StatusCode::BAD_REQUEST,
            Json(ScouterServerError::get_trace_spans_error(e)),
        )
    })?;

    let spans = data
        .trace_service
        .query_service
        .get_trace_spans(
            Some(trace_id_bytes.as_slice()),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .map_err(|e| {
            error!("Failed to get trace spans: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::get_trace_spans_error(e)),
            )
        })?;

    Ok(Json(TraceSpansResponse {
        spans: redact_trace_spans_for_permissions(spans, &perms),
    }))
}

#[utoipa::path(
    get,
    path = "/scouter/trace/spans",
    params(TraceRequest),
    responses(
        (status = 200, description = "Trace spans", body = TraceSpansResponse),
        (status = 400, description = "Invalid trace ID", body = ScouterServerError),
        (status = 500, description = "Internal server error", body = ScouterServerError),
    ),
    tag = "traces",
    security(("bearer_token" = []))
)]
#[instrument(skip_all)]
pub async fn get_trace_spans(
    State(data): State<Arc<AppState>>,
    Extension(perms): Extension<UserPermissions>,
    Query(params): Query<TraceRequest>,
) -> Result<Json<TraceSpansResponse>, (StatusCode, Json<ScouterServerError>)> {
    debug!(
        "Getting trace spans for trace_id: {}, service_name: {:?}",
        params.trace_id, params.service_name,
    );

    let trace_id_bytes = TraceId::hex_to_bytes(&params.trace_id).map_err(|e| {
        error!("Invalid trace_id hex: {:?}", e);
        (
            StatusCode::BAD_REQUEST,
            Json(ScouterServerError::get_trace_spans_error(e)),
        )
    })?;

    // Parse caller-supplied time bounds or default to a ±24h window.
    // Time-first predicates narrow the Delta Lake file scan before the trace_id filter.
    let end_time = params
        .end_time
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(chrono::Utc::now);
    let start_time = params
        .start_time
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|| end_time - chrono::Duration::hours(24));

    let spans = data
        .trace_service
        .query_service
        .get_trace_spans(
            Some(trace_id_bytes.as_slice()),
            params.service_name.as_deref(),
            params.service_namespace.as_deref(),
            params.service_version.as_deref(),
            params.service_instance_id.as_deref(),
            Some(&start_time),
            Some(&end_time),
            None,
        )
        .await
        .map_err(|e| {
            error!("Failed to get trace spans: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::get_trace_spans_error(e)),
            )
        })?;

    Ok(Json(TraceSpansResponse {
        spans: redact_trace_spans_for_permissions(spans, &perms),
    }))
}

#[utoipa::path(
    post,
    path = "/scouter/trace/spans/tags",
    request_body = SpansFromTagsRequest,
    responses(
        (status = 200, description = "Trace spans matching tags", body = TraceSpansResponse),
        (status = 500, description = "Internal server error", body = ScouterServerError),
    ),
    tag = "traces",
    security(("bearer_token" = []))
)]
#[instrument(skip_all)]
pub async fn query_trace_spans_from_tags(
    State(data): State<Arc<AppState>>,
    Extension(perms): Extension<UserPermissions>,
    Json(params): Json<SpansFromTagsRequest>,
) -> Result<Json<TraceSpansResponse>, (StatusCode, Json<ScouterServerError>)> {
    // Step 1: resolve tags → trace_id hex strings via PostgreSQL
    let tags: Vec<Tag> = params
        .tag_filters
        .iter()
        .filter_map(|m| {
            Some(Tag {
                key: m.get("key")?.clone(),
                value: m.get("value")?.clone(),
            })
        })
        .collect();

    let trace_id_hexes = PostgresClient::get_entity_id_by_tags(
        &data.db_pool,
        &params.entity_type,
        &tags,
        params.match_all,
    )
    .await
    .map_err(|e| {
        error!("Failed to get entity IDs from tags: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ScouterServerError::get_trace_spans_error(e)),
        )
    })?;

    // Step 2: fetch spans from Delta Lake for each trace_id
    let mut all_spans = Vec::new();
    for hex_id in &trace_id_hexes {
        let trace_id_bytes = TraceId::hex_to_bytes(hex_id).map_err(|e| {
            error!("Invalid trace_id hex from tags: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::get_trace_spans_error(e)),
            )
        })?;
        let spans = data
            .trace_service
            .query_service
            .get_trace_spans(
                Some(trace_id_bytes.as_slice()),
                params.service_name.as_deref(),
                params.service_namespace.as_deref(),
                params.service_version.as_deref(),
                params.service_instance_id.as_deref(),
                None,
                None,
                None,
            )
            .await
            .map_err(|e| {
                error!("Failed to get trace spans from Delta Lake: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ScouterServerError::get_trace_spans_error(e)),
                )
            })?;
        all_spans.extend(spans);
    }

    Ok(Json(TraceSpansResponse {
        spans: redact_trace_spans_for_permissions(all_spans, &perms),
    }))
}

// Get trace metrics aggregated into buckets by time interval, optionally filtered by request-body query.
#[utoipa::path(
    post,
    path = "/scouter/trace/metrics",
    request_body = TraceMetricsRequest,
    responses(
        (status = 200, description = "Trace metrics", body = TraceMetricsResponse),
        (status = 500, description = "Internal server error", body = ScouterServerError),
    ),
    tag = "traces",
    security(("bearer_token" = []))
)]
#[instrument(skip_all)]
pub async fn trace_metrics(
    State(data): State<Arc<AppState>>,
    Json(body): Json<TraceMetricsRequest>,
) -> Result<Json<TraceMetricsResponse>, (StatusCode, Json<ScouterServerError>)> {
    let body = normalize_metrics_request(body)?;
    if let Some(clause) = &body.clause {
        validate_clause(clause)
            .map_err(|msg| (StatusCode::BAD_REQUEST, Json(ScouterServerError::new(msg))))?;
    }
    debug!(
        "trace_metrics: start={:?} end={:?} interval={}",
        body.start_time, body.end_time, body.bucket_interval
    );

    // Normalize legacy interval strings like "1 minutes" → "minute" for DataFusion DATE_TRUNC.
    let bucket_interval = body
        .bucket_interval
        .split_whitespace()
        .last()
        .unwrap_or(&body.bucket_interval)
        .trim_end_matches('s')
        .to_string();

    // entity_uid is applied as a direct column predicate on `entity_id` inside DataFusion,
    // enabling Z-ORDER file skipping without a Postgres trace_id lookup round-trip.
    let metrics = data
        .trace_service
        .query_service
        .get_trace_metrics(&body, &bucket_interval)
        .await
        .map_err(|e| {
            error!("Failed to get trace metrics: {:?}", e);
            let (status, message) = trace_query_status(&e);
            (
                status,
                Json(match message {
                    Some(message) => ScouterServerError::new(message),
                    None => ScouterServerError::get_trace_metrics_error(e),
                }),
            )
        })?;

    Ok(Json(TraceMetricsResponse { metrics }))
}

#[utoipa::path(
    post,
    path = "/scouter/trace/facets",
    request_body = TraceFilters,
    params(("q" = Option<String>, Query, description = "Optional Lucene-style search query")),
    responses(
        (status = 200, description = "Trace facets", body = TraceFacetsResponse),
        (status = 500, description = "Internal server error", body = ScouterServerError),
    ),
    tag = "traces",
    security(("bearer_token" = []))
)]
#[instrument(skip_all)]
pub async fn get_trace_facets(
    State(data): State<Arc<AppState>>,
    Query(search): Query<SearchQ>,
    Json(body): Json<TraceFilters>,
) -> Result<Json<TraceFacetsResponse>, (StatusCode, Json<ScouterServerError>)> {
    let body = merge_q_into_filters(search.q, body)?;
    validate_filters(&body)?;
    debug!(
        "trace_facets: limit={:?} start={:?} end={:?}",
        body.limit, body.start_time, body.end_time
    );
    let facets = data
        .trace_summary_service
        .query_service
        .get_trace_facets(&body)
        .await
        .map_err(|e| {
            error!("Failed to get trace facets: {:?}", e);
            let (status, message) = trace_query_status(&e);
            (
                status,
                Json(match message {
                    Some(message) => ScouterServerError::new(message),
                    None => ScouterServerError::get_trace_facets_error(e),
                }),
            )
        })?;
    Ok(Json(facets))
}

#[utoipa::path(
    post,
    path = "/scouter/trace/spans/filters",
    request_body = TraceFilters,
    params(("q" = Option<String>, Query, description = "Optional Lucene-style search query")),
    responses(
        (status = 200, description = "Trace spans matching filters", body = TraceSpansResponse),
        (status = 500, description = "Internal server error", body = ScouterServerError),
    ),
    tag = "traces",
    security(("bearer_token" = []))
)]
#[instrument(skip_all)]
pub async fn query_spans_from_filters(
    State(data): State<Arc<AppState>>,
    Extension(perms): Extension<UserPermissions>,
    Query(search): Query<SearchQ>,
    Json(body): Json<TraceFilters>,
) -> Result<Json<TraceSpansResponse>, (StatusCode, Json<ScouterServerError>)> {
    let body = merge_q_into_filters(search.q, body)?;
    validate_filters(&body)?;
    let spans = data
        .trace_service
        .query_service
        .query_spans_from_trace_filters(&body)
        .await
        .map_err(|e| {
            error!("Failed to get spans from trace filters: {:?}", e);
            let (status, message) = trace_query_status(&e);
            (
                status,
                Json(match message {
                    Some(message) => ScouterServerError::new(message),
                    None => ScouterServerError::get_trace_spans_error(e),
                }),
            )
        })?;

    Ok(Json(TraceSpansResponse {
        spans: redact_trace_spans_for_permissions(spans, &perms),
    }))
}

#[utoipa::path(
    post,
    path = "/scouter/v1/traces",
    request_body(
        content = Vec<u8>,
        content_type = "application/x-protobuf",
        description = "OTLP ExportTraceServiceRequest (protobuf-encoded)"
    ),
    responses(
        (status = 200, description = "Spans accepted (protobuf ExportTraceServiceResponse)"),
        (status = 400, description = "Invalid protobuf body", body = ScouterServerError),
        (status = 415, description = "Unsupported media type", body = ScouterServerError),
        (status = 429, description = "Ingest channel full", body = ScouterServerError),
        (status = 500, description = "Internal server error", body = ScouterServerError),
    ),
    tag = "traces"
)]
#[instrument(skip_all)]
pub async fn v1_otel_traces(
    State(data): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<axum::response::Response, (StatusCode, Json<ScouterServerError>)> {
    let content_type = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/x-protobuf");

    if !content_type.contains("application/x-protobuf") {
        return Err((
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Json(ScouterServerError::new(
                "OTLP/HTTP requires Content-Type: application/x-protobuf".to_string(),
            )),
        ));
    }

    let request = ExportTraceServiceRequest::decode(body).map_err(|e| {
        error!("Failed to decode OTLP protobuf body: {:?}", e);
        (
            StatusCode::BAD_REQUEST,
            Json(ScouterServerError::new(format!(
                "Invalid protobuf body: {e}"
            ))),
        )
    })?;

    data.trace_record_tx
        .try_send(TraceServerRecord { request })
        .map_err(|e| {
            let status = match e {
                flume::TrySendError::Full(_) => StatusCode::TOO_MANY_REQUESTS,
                flume::TrySendError::Disconnected(_) => StatusCode::INTERNAL_SERVER_ERROR,
            };
            error!("Failed to enqueue OTLP trace spans: {:?}", e);
            (
                status,
                Json(ScouterServerError::new(
                    "Failed to enqueue trace spans".to_string(),
                )),
            )
        })?;

    let response_bytes = ExportTraceServiceResponse::default().encode_to_vec();
    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/x-protobuf")],
        response_bytes,
    )
        .into_response())
}

#[cfg(debug_assertions)]
#[utoipa::path(
    get,
    path = "/scouter/trace/debug/recent",
    responses(
        (status = 200, description = "Recent traces (last 24h, limit 10)", body = TracePaginationResponse),
        (status = 500, description = "Internal server error", body = ScouterServerError),
    ),
    tag = "traces",
    security(("bearer_token" = []))
)]
#[cfg(debug_assertions)]
#[instrument(skip_all)]
pub async fn debug_recent_traces(
    State(data): State<Arc<AppState>>,
) -> Result<Json<TracePaginationResponse>, (StatusCode, Json<ScouterServerError>)> {
    let end_time = chrono::Utc::now();
    let start_time = end_time - chrono::Duration::hours(24);

    let filters = TraceFilters {
        start_time: Some(start_time),
        end_time: Some(end_time),
        limit: Some(10),
        ..Default::default()
    };

    let response = data
        .trace_summary_service
        .query_service
        .get_paginated_traces(&filters)
        .await
        .map_err(|e| {
            error!("Failed to get debug recent traces: {:?}", e);
            let (status, message) = trace_query_status(&e);
            (
                status,
                Json(match message {
                    Some(message) => ScouterServerError::new(message),
                    None => ScouterServerError::get_paginated_traces_error(e),
                }),
            )
        })?;

    Ok(Json(response))
}

pub async fn get_trace_router(prefix: &str) -> Result<Router<Arc<AppState>>> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        let router = Router::new()
            .route(&format!("{prefix}/trace/baggage"), get(get_trace_baggage))
            .route(&format!("{prefix}/trace/paginated"), post(paginated_traces))
            .route(&format!("{prefix}/trace/spans"), get(get_trace_spans))
            .route(
                &format!("{prefix}/trace/spans/tags"),
                post(query_trace_spans_from_tags),
            )
            .route(
                &format!("{prefix}/trace/spans/filters"),
                post(query_spans_from_filters),
            )
            .route(&format!("{prefix}/trace/metrics"), post(trace_metrics))
            .route(&format!("{prefix}/trace/facets"), post(get_trace_facets))
            .route(&format!("{prefix}/v1/traces"), post(v1_otel_traces))
            .route(
                &(format!("{prefix}/v1/traces/") + "{id}/spans"),
                get(get_trace_spans_by_id),
            );

        #[cfg(debug_assertions)]
        let router = router.route(
            &format!("{prefix}/trace/debug/recent"),
            get(debug_recent_traces),
        );

        router
    }));

    match result {
        Ok(router) => Ok(router),
        Err(_) => {
            // panic
            Err(anyhow::anyhow!("Failed to create tag router"))
                .context("Panic occurred while creating the router")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use scouter_types::trace::query::FilterClause;

    fn make_filters(start_time: Option<chrono::DateTime<Utc>>) -> TraceFilters {
        TraceFilters {
            start_time,
            ..Default::default()
        }
    }

    #[test]
    fn body_start_time_beats_dsl() {
        let body_ts = Utc::now();
        let dsl_ts = body_ts - chrono::Duration::hours(1);

        let parsed = make_filters(Some(dsl_ts));
        let body = make_filters(Some(body_ts));

        let merged = merge_filters(parsed, body);
        // body.start_time wins over parsed.start_time
        assert_eq!(merged.start_time, Some(body_ts));
    }

    #[test]
    fn dsl_start_time_used_when_body_absent() {
        let dsl_ts = Utc::now();

        let parsed = make_filters(Some(dsl_ts));
        let body = make_filters(None);

        let merged = merge_filters(parsed, body);
        assert_eq!(merged.start_time, Some(dsl_ts));
    }

    #[test]
    fn trace_ids_union_dedup() {
        let parsed = TraceFilters {
            trace_ids: Some(vec!["aaa".into(), "bbb".into()]),
            ..Default::default()
        };
        let body = TraceFilters {
            trace_ids: Some(vec!["bbb".into(), "ccc".into()]),
            ..Default::default()
        };

        let merged = merge_filters(parsed, body);
        let mut ids = merged.trace_ids.unwrap();
        ids.sort();
        assert_eq!(ids, vec!["aaa", "bbb", "ccc"]);
    }

    #[test]
    fn empty_q_returns_body_unchanged() {
        let body = TraceFilters {
            limit: Some(25),
            ..Default::default()
        };
        let result = merge_q_into_filters(None, body.clone()).unwrap();
        assert_eq!(result.limit, Some(25));
        assert!(result.clause.is_none());
    }

    #[test]
    fn metrics_body_query_merges_supported_fields_and_keeps_body_precedence() {
        let start_time = Utc::now();
        let end_time = start_time + chrono::Duration::hours(1);
        let body = TraceMetricsRequest {
            start_time,
            end_time,
            bucket_interval: "hour".to_string(),
            clause: Some(FilterClause::Service("checkout".to_string())),
            entity_uid: Some("body-entity".to_string()),
            query: Some(
                "start_time:2026-01-01T00:00:00Z entity_uid:query-entity component:kafka"
                    .to_string(),
            ),
        };

        let result = normalize_metrics_request(body).unwrap();
        assert_eq!(result.start_time, start_time);
        assert_eq!(result.end_time, end_time);
        assert_eq!(result.bucket_interval, "hour");
        assert_eq!(result.entity_uid.as_deref(), Some("body-entity"));
        assert!(matches!(result.clause, Some(FilterClause::And(parts)) if parts.len() == 2));
        assert!(result.query.is_none());
    }

    #[test]
    fn validate_clause_rejects_impossible_duration_range() {
        let clause = FilterClause::And(vec![
            FilterClause::DurationMinMs(500),
            FilterClause::DurationMaxMs(100),
        ]);
        assert!(validate_clause(&clause).is_err());
    }

    #[test]
    fn validate_clause_accepts_valid_duration_range() {
        let clause = FilterClause::And(vec![
            FilterClause::DurationMinMs(100),
            FilterClause::DurationMaxMs(500),
        ]);
        assert!(validate_clause(&clause).is_ok());
    }

    #[test]
    fn validate_direction_rejects_unknown() {
        let filters = TraceFilters {
            direction: Some("sideways".into()),
            ..Default::default()
        };
        assert!(validate_filters(&filters).is_err());
    }

    #[test]
    fn validate_direction_accepts_next_and_previous() {
        for dir in ["next", "previous"] {
            let filters = TraceFilters {
                direction: Some(dir.into()),
                ..Default::default()
            };
            assert!(
                validate_filters(&filters).is_ok(),
                "failed for direction={dir}"
            );
        }
    }
}
