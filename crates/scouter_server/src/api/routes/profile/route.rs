

use scouter_contracts::{ProfileRequest,ProfileStatusRequest, ServiceInfo};

use scouter_types::DriftProfile;

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
use axum::{routing::{post, put}, Router};
use anyhow::{Context, Result};
use std::panic::{catch_unwind, AssertUnwindSafe};


pub async fn insert_drift_profile(
    State(data): State<Arc<AppState>>,
    Json(body): Json<ProfileRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    // validate profile is correct
    // this will be used to validate different versions of the drift profile in the future

    let body = DriftProfile::from_value(body.profile, body.drift_type.to_string());

    if body.is_err() {
        // future: - validate against older versions of the drift profile
        let json_response = json!({
            "status": "error",
            "message": "Invalid drift profile"
        });
        return Err((StatusCode::BAD_REQUEST, Json(json_response)));
    }

    let query_result = &data.db.insert_drift_profile(&body.unwrap()).await;

    match query_result {
        Ok(_) => {
            let json_response = json!({
                "status": "success",
                "message": "Monitor profile inserted successfully"
            });
            Ok(Json(json_response))
        }
        Err(e) => {
            error!("Failed to insert monitor profile: {:?}", e);
            let json_response = json!({
                "status": "error",
                "message": format!("{:?}", e)
            });
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json_response)))
        }
    }
}

/// Route to update a drift profile
/// This route will update a drift profile in the database
///
/// # Arguments
///
/// * `data` - Arc<AppState> - Application state
/// * `body` - Json<ProfileRequest> - Profile request
///
pub async fn update_drift_profile(
    State(data): State<Arc<AppState>>,
    Json(body): Json<ProfileRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    // validate profile is correct
    // this will be used to validate different versions of the drift profile in the future
    let body = DriftProfile::from_value(body.profile, body.drift_type.to_string());

    if body.is_err() {
        // future: - validate against older versions of the drift profile
        let json_response = json!({
            "status": "error",
            "message": "Invalid drift profile"
        });
        return Err((StatusCode::BAD_REQUEST, Json(json_response)));
    }

    let query_result = &data.db.update_drift_profile(&body.unwrap()).await;

    match query_result {
        Ok(_) => {
            let json_response = json!({
                "status": "success",
                "message": "Drift profile updated successfully"
            });
            Ok(Json(json_response))
        }
        Err(e) => {
            error!("Failed to update drift profile: {:?}", e);
            let json_response = json!({
                "status": "error",
                "message": format!("{:?}", e)
            });
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json_response)))
        }
    }
}

/// Retrieve a drift profile from the database
///
/// # Arguments
///
/// * `data` - Arc<AppState> - Application state
/// * `params` - Query<ServiceInfo> - Query parameters
///
/// # Returns
///
/// * `Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)>` - Result of the request
pub async fn get_profile(
    State(data): State<Arc<AppState>>,
    params: Query<ServiceInfo>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let profile = &data.db.get_drift_profile(&params).await;

    match profile {
        Ok(Some(result)) => Ok(Json(json!({
            "status": "success",
            "data": result
        }))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({
                "status": "error",
                "message": "Profile not found"
            })),
        )),
        Err(e) => {
            error!("Failed to query drift profile: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "status": "error",
                    "message": format!("{:?}", e)
                })),
            ))
        }
    }
}

/// Update drift profile status
///
/// # Arguments
///
/// * `data` - Arc<AppState> - Application state
/// * `body` - Json<ProfileStatusRequest> - Profile status request
///
/// # Returns
///
/// * `Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)>` - Result of the request
pub async fn update_drift_profile_status(
    State(data): State<Arc<AppState>>,
    Json(body): Json<ProfileStatusRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let query_result = &data.db.update_drift_profile_status(&body).await;

    match query_result {
        Ok(_) => Ok(Json(json!({
            "status": "success",
            "message": format!(
                "Monitor profile status updated to {} for {} {} {}",
                &body.active, &body.name, &body.repository, &body.version
            )
        }))),
        Err(e) => {
            error!(
                "Failed to update drift profile status for {} {} {} : {:?}",
                &body.name, &body.repository, &body.version, e
            );
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "status": "error",
                    "message": format!("{:?}", e)
                })),
            ))
        }
    }
}

pub async fn get_profile_router(prefix: &str) -> Result<Router<Arc<AppState>>> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        Router::new().route(&format!("{}/profile", prefix), post(insert_drift_profile)
                .put(update_drift_profile)
                .get(get_profile)).route(&format!("{}/profile/status", prefix),put(update_drift_profile_status))
    }));

    match result {
        Ok(router) => Ok(router),
        Err(_) => {
            // panic
            Err(anyhow::anyhow!("Failed to create health router"))
                .context("Panic occurred while creating the router")
        }
    }
}