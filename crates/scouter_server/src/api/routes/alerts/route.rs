use crate::api::state::AppState;

use anyhow::{Context, Result};
use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use scouter_sql::sql::traits::AlertSqlLogic;
use scouter_sql::PostgresClient;
use scouter_types::contracts::{
    DriftAlertPaginationRequest, DriftAlertPaginationResponse, ScouterServerError,
    UpdateAlertResponse, UpdateAlertStatus,
};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use tracing::error;
/// Retrieve drift alerts from the database
///
/// # Arguments
///
/// * `data` - Arc<AppState> - Application state
/// * `params` - Query<DriftAlertPaginationRequest> - Query parameters
///
/// # Returns
///
/// * `Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)>` - Result of the request
pub async fn drift_alerts(
    State(data): State<Arc<AppState>>,
    Json(params): Json<DriftAlertPaginationRequest>,
) -> Result<Json<DriftAlertPaginationResponse>, (StatusCode, Json<ScouterServerError>)> {
    let entity_id = data.get_entity_id_for_request(&params.uid).await?;

    let alerts = PostgresClient::get_paginated_drift_alerts(&data.db_pool, &params, &entity_id)
        .await
        .map_err(|e| {
            error!("Failed to query drift alerts: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::query_alerts_error(e)),
            )
        })?;

    Ok(Json(alerts))
}

pub async fn update_alert_status(
    State(data): State<Arc<AppState>>,
    Json(body): Json<UpdateAlertStatus>,
) -> Result<Json<UpdateAlertResponse>, (StatusCode, Json<ScouterServerError>)> {
    let query_result = PostgresClient::update_drift_alert_status(&data.db_pool, &body)
        .await
        .map_err(|e| {
            error!("Failed to update drift alert status: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::new(format!(
                    "Failed to update drift alert status: {e:?}"
                ))),
            )
        })?;

    if query_result.active == body.active {
        Ok(Json(UpdateAlertResponse { updated: true }))
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            Json(ScouterServerError::new(format!(
                "Failed to update drift alert status: {query_result:?}"
            ))),
        ))
    }
}

pub async fn get_alert_router(prefix: &str) -> Result<Router<Arc<AppState>>> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        Router::new().route(
            &format!("{prefix}/alerts"),
            post(drift_alerts).put(update_alert_status),
        )
    }));

    match result {
        Ok(router) => Ok(router),
        Err(_) => {
            // panic
            Err(anyhow::anyhow!("Failed to create drift router"))
                .context("Panic occurred while creating the router")
        }
    }
}
