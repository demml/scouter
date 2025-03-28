use crate::api::state::AppState;
use anyhow::{Context, Result};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use scouter_contracts::{DriftAlertRequest, UpdateAlertStatus};
use serde_json::json;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use tracing::error;

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
    params: Query<DriftAlertRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let query_result = &data.db.get_drift_alerts(&params).await;

    match query_result {
        Ok(result) => {
            let json_response = json!({
                "status": "success",
                "data": result
            });
            Ok(Json(json_response))
        }
        Err(e) => {
            error!("Failed to query drift alerts: {:?}", e);
            let json_response = json!({
                "status": "error",
                "message": format!("{:?}", e)
            });
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json_response)))
        }
    }
}

pub async fn update_alert_status(
    State(data): State<Arc<AppState>>,
    params: Query<UpdateAlertStatus>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let query_result = &data.db.update_drift_alerts(&params).await;

    match query_result {
        Ok(result) => {
            let json_response = json!({
                "status": "success",
                "data": result
            });
            Ok(Json(json_response))
        }
        Err(e) => {
            error!("Failed to update drift alerts: {:?}", e);
            let json_response = json!({
                "status": "error",
                "message": format!("{:?}", e)
            });
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json_response)))
        }
    }
}

pub async fn get_alert_router(prefix: &str) -> Result<Router<Arc<AppState>>> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        Router::new().route(&format!("{}/alerts", prefix), get(get_drift_alerts))
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
