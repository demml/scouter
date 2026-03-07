use crate::api::state::AppState;

use anyhow::{Context, Result};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};

use scouter_sql::sql::traits::TraceSqlLogic;
use scouter_sql::PostgresClient;
use scouter_types::{
    contracts::ScouterServerError, sql::TraceFilters, SpansFromTagsRequest, TraceBaggageResponse,
    TraceId, TraceMetricsRequest, TraceMetricsResponse, TracePaginationResponse,
    TraceReceivedResponse, TraceRequest, TraceSpansResponse,
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
    Json(mut body): Json<TraceFilters>,
) -> Result<Json<TracePaginationResponse>, (StatusCode, Json<ScouterServerError>)> {
    debug!("Getting paginated traces with filters: {:?}", body);

    // ── Resolve entity_uid → trace_ids via Postgres trace_entities ───────────
    // attribute_filters are handled inside get_paginated_traces via JOIN with trace_spans.
    let entity_trace_ids = if let Some(ref uid) = body.entity_uid {
        PostgresClient::get_trace_ids_for_entity(&data.db_pool, uid)
            .await
            .unwrap_or_else(|e| {
                error!("Failed to resolve entity_uid trace IDs: {:?}", e);
                Vec::new()
            })
    } else {
        Vec::new()
    };

    if !entity_trace_ids.is_empty() {
        let hex_ids: Vec<String> = entity_trace_ids.iter().map(hex::encode).collect();
        body.trace_ids = Some(match body.trace_ids {
            Some(existing) if !existing.is_empty() => existing
                .into_iter()
                .filter(|id| hex_ids.contains(id))
                .collect(),
            _ => hex_ids,
        });
    }

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
    // Execute both queries concurrently
    let spans = PostgresClient::get_spans_from_tags(
        &data.db_pool,
        &params.entity_type,
        params.tag_filters,
        params.match_all,
        params.service_name.as_deref(),
    )
    .await
    .map_err(|e| {
        error!("Failed to get trace spans from tags: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ScouterServerError::get_trace_spans_error(e)),
        )
    })?;

    Ok(Json(TraceSpansResponse { spans }))
}

#[instrument(skip_all)]
pub async fn trace_metrics(
    State(data): State<Arc<AppState>>,
    Json(body): Json<TraceMetricsRequest>,
) -> Result<Json<TraceMetricsResponse>, (StatusCode, Json<ScouterServerError>)> {
    debug!("Getting trace metrics for request: {:?}", body);

    // ── Resolve entity_uid → raw trace_id bytes via Postgres trace_entities ──
    let entity_trace_ids = if let Some(ref uid) = body.entity_uid {
        PostgresClient::get_trace_ids_for_entity(&data.db_pool, uid)
            .await
            .unwrap_or_else(|e| {
                error!("Failed to resolve entity_uid trace IDs: {:?}", e);
                Vec::new()
            })
    } else {
        Vec::new()
    };

    let entity_ids_ref: Option<&[Vec<u8>]> = if entity_trace_ids.is_empty() {
        None
    } else {
        Some(&entity_trace_ids)
    };

    let attr_filters_ref: Option<&[String]> =
        body.attribute_filters.as_deref().filter(|f| !f.is_empty());

    let metrics = data
        .trace_service
        .query_service
        .get_trace_metrics(
            body.service_name.as_deref(),
            body.start_time,
            body.end_time,
            &body.bucket_interval,
            attr_filters_ref,
            entity_ids_ref,
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
