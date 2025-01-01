use scouter_contracts::ObservabilityMetricRequest;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

use serde_json::json;
use std::sync::Arc;
use tracing::error;

use crate::api::state::AppState;
use axum::{routing::get, Router};
use anyhow::{Context, Result};
use std::panic::{catch_unwind, AssertUnwindSafe};



pub async fn get_observability_metrics(
    State(data): State<Arc<AppState>>,
    params: Query<ObservabilityMetricRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let query_result = &data.db.get_binned_observability_metrics(&params).await;

    match query_result {
        Ok(result) => {
            let json_response = serde_json::json!({
                "status": "success",
                "data": result
            });
            Ok(Json(json_response))
        }
        Err(e) => {
            error!("Failed to query observability_metrics: {:?}", e);
            let json_response = json!({
                "status": "error",
                "message": format!("{:?}", e)
            });
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json_response)))
        }
    }
}

pub async fn get_observability_router(prefix: &str) -> Result<Router<Arc<AppState>>> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        Router::new().route(&format!("{}/observability/metrics", prefix), get(get_observability_metrics),)
    }));

    match result {
        Ok(router) => Ok(router),
        Err(_) => {
            // panic
            Err(anyhow::anyhow!("Failed to create observability router"))
                .context("Panic occurred while creating the router")
        }
    }
}