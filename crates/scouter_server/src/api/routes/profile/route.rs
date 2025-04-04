use scouter_contracts::{GetProfileRequest, ProfileRequest, ProfileStatusRequest};

use scouter_types::DriftProfile;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};

use serde_json::json;
use std::sync::Arc;
use tracing::{debug, error, instrument};

use crate::api::state::AppState;
use anyhow::{Context, Result};
use axum::{
    routing::{post, put},
    Router,
};
use scouter_auth::permission::UserPermissions;
use std::panic::{catch_unwind, AssertUnwindSafe};

fn make_error_response(
    status: StatusCode,
    message: impl ToString,
) -> (StatusCode, Json<serde_json::Value>) {
    (
        status,
        Json(json!({
            "status": "error",
            "message": message.to_string()
        })),
    )
}

pub async fn insert_drift_profile(
    State(data): State<Arc<AppState>>,
    Extension(perms): Extension<UserPermissions>,
    Json(body): Json<ProfileRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    if !perms.has_write_permission(&body.repository) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Permission denied" })),
        ));
    }

    // validate profile is correct
    // this will be used to validate different versions of the drift profile in the future

    let body = DriftProfile::from_str(body.drift_type, body.profile);

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
    Extension(perms): Extension<UserPermissions>,
    Json(body): Json<ProfileRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    if !perms.has_write_permission(&body.repository) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Permission denied" })),
        ));
    }
    // validate profile is correct
    // this will be used to validate different versions of the drift profile in the future
    let body = DriftProfile::from_str(body.drift_type, body.profile);

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
#[instrument(skip_all)]
pub async fn get_profile(
    State(data): State<Arc<AppState>>,
    Query(params): Query<GetProfileRequest>,
    Extension(perms): Extension<UserPermissions>,
) -> Result<Json<DriftProfile>, (StatusCode, Json<serde_json::Value>)> {
    if !perms.has_read_permission() {
        return Err(make_error_response(
            StatusCode::FORBIDDEN,
            "Permission denied",
        ));
    }

    debug!("Getting drift profile: {:?}", &params);

    let result = data.db.get_drift_profile(&params).await.map_err(|e| {
        error!("Failed to query drift profile: {:?}", e);
        make_error_response(StatusCode::INTERNAL_SERVER_ERROR, e)
    })?;

    let Some(profile_value) = result else {
        return Err(make_error_response(
            StatusCode::NOT_FOUND,
            "Profile not found",
        ));
    };

    let profile =
        DriftProfile::from_value(profile_value, params.drift_type.to_string()).map_err(|e| {
            error!("Failed to parse drift profile: {:?}", e);
            make_error_response(StatusCode::INTERNAL_SERVER_ERROR, e)
        })?;

    Ok(Json(profile))
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
#[instrument(skip(data, body))]
pub async fn update_drift_profile_status(
    State(data): State<Arc<AppState>>,
    Extension(perms): Extension<UserPermissions>,
    Json(body): Json<ProfileStatusRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    if !perms.has_write_permission(&body.repository) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Permission denied" })),
        ));
    }
    debug!("Updating drift profile status: {:?}", &body);

    let query_result = &data.db.update_drift_profile_status(&body).await;

    match query_result {
        Ok(_) => Ok(Json(json!({
            "status": "success",
            "message": format!(
                "Monitor profile status updated to {} for {} {} {}",
                &body.active, &body.name, &body.space, &body.version
            )
        }))),
        Err(e) => {
            error!(
                "Failed to update drift profile status for {} {} {} : {:?}",
                &body.name, &body.space, &body.version, e
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
        Router::new()
            .route(
                &format!("{}/profile", prefix),
                post(insert_drift_profile)
                    .put(update_drift_profile)
                    .get(get_profile),
            )
            .route(
                &format!("{}/profile/status", prefix),
                put(update_drift_profile_status),
            )
    }));

    match result {
        Ok(router) => Ok(router),
        Err(_) => {
            // panic
            Err(anyhow::anyhow!("Failed to create profile router"))
                .context("Panic occurred while creating the router")
        }
    }
}
