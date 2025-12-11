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
    contracts::ScouterServerError, sql::TraceFilters, TraceBaggageResponse, TraceMetricsRequest,
    TraceMetricsResponse, TracePaginationResponse, TraceRequest, TraceSpansResponse,
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
pub async fn get_paginated_traces(
    State(data): State<Arc<AppState>>,
    Json(body): Json<TraceFilters>,
) -> Result<Json<TracePaginationResponse>, (StatusCode, Json<ScouterServerError>)> {
    let pagination_response = PostgresClient::get_traces_paginated(&data.db_pool, body.clone())
        .await
        .map_err(|e| {
            error!("Failed to get paginated traces: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::get_paginated_traces_error(e)),
            )
        })?;

    Ok(Json(pagination_response))
}

#[instrument(skip_all)]
pub async fn get_trace_spans(
    State(data): State<Arc<AppState>>,
    Query(params): Query<TraceRequest>,
) -> Result<Json<TraceSpansResponse>, (StatusCode, Json<ScouterServerError>)> {
    // Execute both queries concurrently
    let spans = PostgresClient::get_trace_spans(
        &data.db_pool,
        &params.trace_id,
        params.service_name.as_deref(),
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
pub async fn get_trace_metrics(
    State(data): State<Arc<AppState>>,
    Query(body): Query<TraceMetricsRequest>,
) -> Result<Json<TraceMetricsResponse>, (StatusCode, Json<ScouterServerError>)> {
    debug!("Getting trace metrics for request: {:?}", body);
    let metrics = PostgresClient::get_trace_metrics(
        &data.db_pool,
        body.service_name.as_deref(),
        body.start_time,
        body.end_time,
        &body.bucket_interval,
    )
    .await
    .map_err(|e| {
        error!("Failed to get trace spans: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ScouterServerError::get_trace_metrics_error(e)),
        )
    })?;

    Ok(Json(TraceMetricsResponse { metrics }))
}

pub async fn get_trace_router(prefix: &str) -> Result<Router<Arc<AppState>>> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        Router::new()
            .route(&format!("{prefix}/trace/baggage"), get(get_trace_baggage))
            .route(
                &format!("{prefix}/trace/paginated"),
                post(get_paginated_traces),
            )
            .route(&format!("{prefix}/trace/spans"), get(get_trace_spans))
            .route(&format!("{prefix}/trace/metrics"), get(get_trace_metrics))
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
