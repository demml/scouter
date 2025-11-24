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
use tracing::error;

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

pub async fn get_trace_spans(
    State(data): State<Arc<AppState>>,
    Query(params): Query<TraceRequest>,
) -> Result<Json<TraceSpansResponse>, (StatusCode, Json<ScouterServerError>)> {
    let spans = PostgresClient::get_trace_spans(&data.db_pool, &params.trace_id)
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

pub async fn get_trace_metrics(
    State(data): State<Arc<AppState>>,
    Query(body): Query<TraceMetricsRequest>,
) -> Result<Json<TraceMetricsResponse>, (StatusCode, Json<ScouterServerError>)> {
    let metrics = PostgresClient::get_trace_metrics(
        &data.db_pool,
        body.space.as_deref(),
        body.name.as_deref(),
        body.version.as_deref(),
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

pub async fn get_refresh_trace_summary(
    State(data): State<Arc<AppState>>,
) -> Result<StatusCode, (StatusCode, Json<ScouterServerError>)> {
    PostgresClient::refresh_trace_summary(&data.db_pool)
        .await
        .map_err(|e| {
            error!("Failed to refresh trace summary: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::refresh_trace_summary_error(e)),
            )
        })?;

    Ok(StatusCode::OK)
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
            .route(
                &format!("{prefix}/trace/refresh-summary"),
                get(get_refresh_trace_summary),
            )
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
