use crate::api::state::AppState;

use anyhow::{Context, Result};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};

use scouter_sql::sql::traits::{TagSqlLogic, TraceSqlLogic};
use scouter_sql::PostgresClient;
use scouter_types::{
    contracts::ScouterServerError, sql::TraceFilters, SpansFromTagsRequest, Tag,
    TraceBaggageResponse, TraceId, TraceMetricsRequest, TraceMetricsResponse,
    TracePaginationResponse, TraceReceivedResponse, TraceRequest, TraceSpansResponse,
};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use tracing::instrument;
use tracing::{debug, error};

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

#[instrument(skip_all)]
pub async fn paginated_traces(
    State(data): State<Arc<AppState>>,
    Json(body): Json<TraceFilters>,
) -> Result<Json<TracePaginationResponse>, (StatusCode, Json<ScouterServerError>)> {
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

#[instrument(skip_all)]
pub async fn trace_metrics(
    State(data): State<Arc<AppState>>,
    Json(body): Json<TraceMetricsRequest>,
) -> Result<Json<TraceMetricsResponse>, (StatusCode, Json<ScouterServerError>)> {
    debug!("Getting trace metrics for request: {:?}", body);

    let attr_filters_ref: Option<&[String]> =
        body.attribute_filters.as_deref().filter(|f| !f.is_empty());

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
        .get_trace_metrics(
            body.service_name.as_deref(),
            body.start_time,
            body.end_time,
            &bucket_interval,
            attr_filters_ref,
            body.entity_uid.as_deref(),
        )
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

#[instrument(skip_all)]
pub async fn v1_otel_traces(
    State(_data): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<TraceReceivedResponse>, (StatusCode, Json<ScouterServerError>)> {
    debug!("Getting trace metrics for request: {:?}", body);

    Ok(Json(TraceReceivedResponse { received: true }))
}

pub async fn get_trace_router(prefix: &str) -> Result<Router<Arc<AppState>>> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        Router::new()
            .route(&format!("{prefix}/trace/baggage"), get(get_trace_baggage))
            .route(&format!("{prefix}/trace/paginated"), post(paginated_traces))
            .route(&format!("{prefix}/trace/spans"), get(get_trace_spans))
            .route(
                &format!("{prefix}/trace/spans/tags"),
                post(query_trace_spans_from_tags),
            )
            .route(&format!("{prefix}/trace/metrics"), post(trace_metrics))
            .route(&format!("{prefix}/v1/traces"), post(v1_otel_traces))
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
