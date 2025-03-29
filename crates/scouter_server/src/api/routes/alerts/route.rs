use crate::api::routes::alerts::types::UpdateAlertResponse;
use crate::api::state::AppState;

use anyhow::{Context, Result};
use axum::routing::post;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use scouter_contracts::{DriftAlertRequest, UpdateAlertStatus};
use serde_json::json;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use tracing::error;

use super::types::Alerts;

/// Retrieve drift alerts from the database
///
/// # Arguments
///
/// * `data` - Arc<AppState> - Application state
/// * `params` - Query<DriftAlertRequest> - Query parameters
///
/// # Returns
///
/// * `Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)>` - Result of the request
pub async fn get_drift_alerts(
    State(data): State<Arc<AppState>>,
    Query(params): Query<DriftAlertRequest>,
) -> Result<Json<Alerts>, (StatusCode, Json<serde_json::Value>)> {
    let alerts = &data.db.get_drift_alerts(&params).await.map_err(|e| {
        error!("Failed to query drift alerts: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"status": "error", "message": format!("{:?}", e)})),
        )
    })?;

    Ok(Json(Alerts {
        alerts: alerts.clone(),
    }))
}

pub async fn update_alert_status(
    State(data): State<Arc<AppState>>,
    Json(body): Json<UpdateAlertStatus>,
) -> Result<Json<UpdateAlertResponse>, (StatusCode, Json<serde_json::Value>)> {
    let query_result = &data
        .db
        .update_drift_alert_status(&body)
        .await
        .map_err(|e| {
            error!("Failed to update drift alert status: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"status": "error", "message": format!("{:?}", e)})),
            )
        })?;

    if query_result.active == body.active {
        return Ok(Json(UpdateAlertResponse { updated: true }));
    } else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"status": "error", "message": "Alert status update failed"})),
        ));
    }
}

pub async fn get_alert_router(prefix: &str) -> Result<Router<Arc<AppState>>> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        Router::new().route(
            &format!("{}/alerts", prefix),
            get(get_drift_alerts).put(update_alert_status),
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
