use scouter_types::contracts::{
    GetProfileRequest, ProfileRequest, ProfileStatusRequest, RegisteredProfileResponse,
    ScouterResponse, ScouterServerError,
};

use scouter_types::DriftProfile;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    Extension, Json,
};

use std::sync::Arc;
use tracing::{debug, error, info, instrument};

use crate::api::state::AppState;
use anyhow::{Context, Result};
use axum::{
    routing::{post, put},
    Router,
};
use scouter_auth::permission::UserPermissions;
use scouter_sql::sql::traits::ProfileSqlLogic;
use scouter_sql::PostgresClient;
use std::panic::{catch_unwind, AssertUnwindSafe};

/// Insert a drift profile into the database
#[instrument(skip_all)]
pub async fn insert_drift_profile(
    State(data): State<Arc<AppState>>,
    Extension(perms): Extension<UserPermissions>,
    Json(request): Json<ProfileRequest>,
) -> Result<Json<RegisteredProfileResponse>, (StatusCode, Json<ScouterServerError>)> {
    if !perms.has_write_permission(&request.space) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ScouterServerError::permission_denied()),
        ));
    }

    info!(
        "Inserting drift profile: {:?} {:?}",
        &request.space, &request.drift_type
    );

    // validate profile is correct
    // this will be used to validate different versions of the drift profile in the future
    let body = match DriftProfile::from_str(request.drift_type, request.profile) {
        Ok(profile) => profile,
        Err(e) => {
            error!("Failed to parse drift profile: {:?}", e);
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ScouterServerError::new(format!(
                    "Invalid drift profile: {e:?}",
                ))),
            ));
        }
    };

    // get versions
    let base_args = body.get_base_args();

    let version = match PostgresClient::get_next_profile_version(
        &data.db_pool,
        &base_args,
        request.version_request.version_type,
        request.version_request.pre_tag,
        request.version_request.build_tag,
    )
    .await
    {
        Ok(version) => version,
        Err(e) => {
            error!("Failed to get next profile version: {:?}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::new(format!(
                    "Failed to get next profile version: {e:?}",
                ))),
            ));
        }
    };

    match PostgresClient::insert_drift_profile(&data.db_pool, &body, &base_args, &version).await {
        Ok(_) => Ok(Json(RegisteredProfileResponse {
            space: base_args.space,
            name: base_args.name,
            version: version.to_string(),
            status: "success".to_string(),
        })),
        Err(e) => {
            error!("Failed to insert drift profile: {:?}", e);

            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::new(format!(
                    "Failed to insert monitor profile: {e:?}",
                ))),
            ))
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
#[instrument(skip_all)]
pub async fn update_drift_profile(
    State(data): State<Arc<AppState>>,
    Extension(perms): Extension<UserPermissions>,
    Json(body): Json<ProfileRequest>,
) -> Result<Json<ScouterResponse>, (StatusCode, Json<ScouterServerError>)> {
    if !perms.has_write_permission(&body.space) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ScouterServerError::permission_denied()),
        ));
    }
    // validate profile is correct
    // this will be used to validate different versions of the drift profile in the future
    let body = match DriftProfile::from_str(body.drift_type, body.profile) {
        Ok(profile) => profile,
        Err(e) => {
            error!("Failed to parse drift profile: {:?}", e);
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ScouterServerError::new(format!(
                    "Invalid drift profile: {e:?}"
                ))),
            ));
        }
    };

    match PostgresClient::update_drift_profile(&data.db_pool, &body).await {
        Ok(_) => Ok(Json(ScouterResponse {
            status: "success".to_string(),
            message: "Drift profile updated successfully".to_string(),
        })),
        Err(e) => {
            error!("Failed to update drift profile: {:?}", e);

            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::new(format!(
                    "Failed to update drift profile: {e:?}",
                ))),
            ))
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
) -> Result<Json<DriftProfile>, (StatusCode, Json<ScouterServerError>)> {
    if !perms.has_read_permission(&params.space) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ScouterServerError::permission_denied()),
        ));
    }

    debug!("Getting drift profile: {:?}", &params);

    let profile_value = match PostgresClient::get_drift_profile(&data.db_pool, &params).await {
        Ok(Some(value)) => value,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ScouterServerError::new(
                    "Drift profile not found".to_string(),
                )),
            ))
        }
        Err(e) => {
            error!("Failed to query drift profile: {:?}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::query_profile_error(e)),
            ));
        }
    };

    match DriftProfile::from_value(profile_value) {
        Ok(profile) => Ok(Json(profile)),
        Err(e) => {
            error!("Failed to parse drift profile: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::new(format!(
                    "Failed to parse drift profile: {e:?}",
                ))),
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
#[instrument(skip_all)]
pub async fn update_drift_profile_status(
    State(data): State<Arc<AppState>>,
    Extension(perms): Extension<UserPermissions>,
    Json(body): Json<ProfileStatusRequest>,
) -> Result<Json<ScouterResponse>, (StatusCode, Json<ScouterServerError>)> {
    if !perms.has_write_permission(&body.space) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ScouterServerError::permission_denied()),
        ));
    }
    info!("Updating drift profile status: {:?}", &body);

    let query_result = PostgresClient::update_drift_profile_status(&data.db_pool, &body).await;

    match query_result {
        Ok(_) => Ok(Json(ScouterResponse {
            status: "success".to_string(),
            message: format!(
                "Monitor profile status updated to {} for {} {} {}",
                &body.active, &body.name, &body.space, &body.version
            ),
        })),
        Err(e) => {
            error!(
                "Failed to update drift profile status for {} {} {} : {:?}",
                &body.name, &body.space, &body.version, e
            );
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::new(format!(
                    "Failed to update drift profile status: {e:?}",
                ))),
            ))
        }
    }
}

pub async fn get_profile_router(prefix: &str) -> Result<Router<Arc<AppState>>> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        Router::new()
            .route(
                &format!("{prefix}/profile"),
                post(insert_drift_profile)
                    .put(update_drift_profile)
                    .get(get_profile),
            )
            .route(
                &format!("{prefix}/profile/status"),
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
