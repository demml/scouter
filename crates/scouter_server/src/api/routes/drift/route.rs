use crate::api::state::AppState;
use anyhow::{Context, Result};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use scouter_contracts::DriftRequest;
use scouter_types::{ServerRecords, ToDriftRecords};
use serde_json::json;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use tracing::error;

pub async fn get_drift(
    State(data): State<Arc<AppState>>,
    Query(params): Query<DriftRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    // validate time window

    let query_result = &data.db.get_binned_drift_records(&params).await;

    match query_result {
        Ok(result) => {
            let json_response = serde_json::json!(result);
            Ok(Json(json_response))
        }
        Err(e) => {
            error!("Failed to query drift records: {:?}", e);
            let json_response = json!({
                "status": "error",
                "message": format!("{:?}", e)
            });
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json_response)))
        }
    }
}

pub async fn insert_drift(
    State(data): State<Arc<AppState>>,
    Json(body): Json<ServerRecords>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let records = body.to_spc_drift_records().map_err(|e| {
        error!("Failed to convert drift records: {:?}", e);
        (
            StatusCode::BAD_REQUEST,
            json!({ "status": "error", "message": format!("{:?}", e) }),
        )
    });

    if records.is_err() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "status": "error", "message": "Invalid drift record" })),
        ));
    }

    for record in records.unwrap() {
        let _ = &data
            .db
            .insert_spc_drift_record(&record)
            .await
            .map_err(|e| {
                error!("Failed to insert drift record: {:?}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "status": "error", "message": format!("{:?}", e) })),
                )
            })?;
    }

    Ok(Json(json!({
        "status": "success",
        "message": "Record inserted successfully"
    })))
}

pub async fn get_drift_router(prefix: &str) -> Result<Router<Arc<AppState>>> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        Router::new().route(
            &format!("{}/drift", prefix),
            get(get_drift).post(insert_drift),
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
