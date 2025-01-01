use crate::api::schema::{
    DriftAlertRequest, DriftRequest, ObservabilityMetricRequest, ProfileRequest,
    ProfileStatusRequest, ServiceInfo,
};
use crate::consumer::base::ToDriftRecords;
use scouter_drift::DriftProfile;
use scouter_types::ServerRecords;

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

pub async fn health_check() -> impl IntoResponse {
    const MESSAGE: &str = "Alive";

    let json_response = serde_json::json!({
        "status": "success",
        "message": MESSAGE
    });

    Json(json_response)
}

pub async fn get_drift(
    State(data): State<Arc<AppState>>,
    params: Query<DriftRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    // validate time window

    let query_result = &data.db.get_binned_drift_records(&params).await;

    match query_result {
        Ok(result) => {
            let json_response = serde_json::json!({
                "status": "success",
                "data": result
            });
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
    let record = body.to_spc_drift_records().map_err(|e| {
        error!("Failed to convert drift records: {:?}", e);
        (
            StatusCode::BAD_REQUEST,
            json!({ "status": "error", "message": format!("{:?}", e) }),
        )
    });

    if record.is_err() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "status": "error", "message": "Invalid drift record" })),
        ));
    }

    let query_result = &data.db.insert_spc_drift_record(&record.unwrap()[0]).await;

    match query_result {
        Ok(_) => Ok(Json(json!({
            "status": "success",
            "message": "Record inserted successfully"
        }))),
        Err(e) => {
            error!("Failed to insert drift record: {:?}", e);
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

pub async fn insert_drift_profile(
    State(data): State<Arc<AppState>>,
    Json(body): Json<ProfileRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    // validate profile is correct
    // this will be used to validate different versions of the drift profile in the future

    let body = DriftProfile::from_value(body.profile, body.drift_type.value());

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
    let body = DriftProfile::from_value(body.profile, body.drift_type.value());

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
