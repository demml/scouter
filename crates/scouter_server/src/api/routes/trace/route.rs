use crate::api::state::AppState;

use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::{Query, State},
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
use scouter_sql::PostgresClient;
use scouter_sql::sql::traits::{TagSqlLogic, TraceSqlLogic};
use scouter_types::{
    SpansFromTagsRequest, Tag, TraceBaggageResponse, TraceFacetsResponse, TraceId,
    TraceMetricsRequest, TraceMetricsResponse, TracePaginationResponse, TraceRequest,
    TraceServerRecord, TraceSpansResponse,
    contracts::ScouterServerError,
    sql::TraceFilters,
    trace::query::{FilterClause, parse_search_query},
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
        queue_uid: body.queue_uid.or(parsed.queue_uid),
    }
}

fn merge_q_into_metrics(
    q: Option<String>,
    body: TraceMetricsRequest,
) -> Result<TraceMetricsRequest, (StatusCode, Json<ScouterServerError>)> {
    let Some(q) = q.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(body);
    };
    let parsed = parse_search_query(q).map_err(invalid_search_query)?;
    Ok(TraceMetricsRequest {
        start_time: body.start_time,
        end_time: body.end_time,
        bucket_interval: body.bucket_interval,
        clause: FilterClause::and_merge(parsed.clause, body.clause),
        entity_uid: body.entity_uid.or(parsed.entity_uid),
    })
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
    debug!("Getting paginated traces with filters: {:?}", body);

    // entity_uid is passed directly to the Delta Lake query where it is applied as a
    // column predicate on the `entity_id` column, enabling file-level Z-ORDER skipping.
    let pagination_response = data
        .trace_summary_service
        .query_service
        .get_paginated_traces(&body)
        .await
        .map_err(|e| {
            error!("Failed to get paginated traces: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::get_paginated_traces_error(e)),
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

    Ok(Json(TraceSpansResponse { spans }))
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

    Ok(Json(TraceSpansResponse { spans }))
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

    Ok(Json(TraceSpansResponse { spans: all_spans }))
}

#[utoipa::path(
    post,
    path = "/scouter/trace/metrics",
    request_body = TraceMetricsRequest,
    params(("q" = Option<String>, Query, description = "Optional Lucene-style search query")),
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
    Query(search): Query<SearchQ>,
    Json(body): Json<TraceMetricsRequest>,
) -> Result<Json<TraceMetricsResponse>, (StatusCode, Json<ScouterServerError>)> {
    let body = merge_q_into_metrics(search.q, body)?;
    debug!("Getting trace metrics for request: {:?}", body);

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
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::get_trace_metrics_error(e)),
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
    debug!("Getting trace facets with filters: {:?}", body);
    let facets = data
        .trace_summary_service
        .query_service
        .get_trace_facets(&body)
        .await
        .map_err(|e| {
            error!("Failed to get trace facets: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::get_trace_facets_error(e)),
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
    Query(search): Query<SearchQ>,
    Json(body): Json<TraceFilters>,
) -> Result<Json<TraceSpansResponse>, (StatusCode, Json<ScouterServerError>)> {
    let body = merge_q_into_filters(search.q, body)?;
    let spans = data
        .trace_service
        .query_service
        .query_spans_from_trace_filters(&body)
        .await
        .map_err(|e| {
            error!("Failed to get spans from trace filters: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::get_trace_spans_error(e)),
            )
        })?;

    Ok(Json(TraceSpansResponse { spans }))
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

#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
pub struct SearchTracesParams {
    pub q: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub limit: Option<i32>,
    pub cursor_start_time: Option<String>,
    pub cursor_trace_id: Option<String>,
    pub direction: Option<String>,
}

#[utoipa::path(
    get,
    path = "/scouter/v1/traces/search",
    params(SearchTracesParams),
    responses(
        (status = 200, description = "Paginated traces matching search query", body = TracePaginationResponse),
        (status = 400, description = "Invalid search query", body = ScouterServerError),
        (status = 500, description = "Internal server error", body = ScouterServerError),
    ),
    tag = "traces",
    security(("bearer_token" = []))
)]
#[instrument(skip_all)]
pub async fn search_traces(
    State(data): State<Arc<AppState>>,
    Query(params): Query<SearchTracesParams>,
) -> Result<Json<TracePaginationResponse>, (StatusCode, Json<ScouterServerError>)> {
    let mut filters = parse_search_query(&params.q).map_err(invalid_search_query)?;

    let parse_ts = |value: &str| {
        chrono::DateTime::parse_from_rfc3339(value)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ScouterServerError::new(format!("Invalid timestamp: {e}"))),
                )
            })
    };

    if let Some(start_time) = params.start_time.as_deref() {
        filters.start_time = Some(parse_ts(start_time)?);
    }
    if let Some(end_time) = params.end_time.as_deref() {
        filters.end_time = Some(parse_ts(end_time)?);
    }
    if let Some(cursor_start_time) = params.cursor_start_time.as_deref() {
        filters.cursor_start_time = Some(parse_ts(cursor_start_time)?);
    }
    if let Some(cursor_trace_id) = params.cursor_trace_id {
        filters.cursor_trace_id = Some(cursor_trace_id);
    }
    if let Some(direction) = params.direction {
        filters.direction = Some(direction);
    }
    if let Some(limit) = params.limit {
        filters.limit = Some(limit);
    }

    let response = data
        .trace_summary_service
        .query_service
        .get_paginated_traces(&filters)
        .await
        .map_err(|e| {
            error!("search_traces failed: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::get_paginated_traces_error(e)),
            )
        })?;

    Ok(Json(response))
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
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::get_paginated_traces_error(e)),
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
            .route(&format!("{prefix}/v1/traces/search"), get(search_traces))
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
