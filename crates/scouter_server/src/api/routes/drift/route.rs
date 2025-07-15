use crate::api::error::ServerError;
use crate::api::state::AppState;
use anyhow::{Context, Result};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Extension, Json, Router,
};
use scouter_auth::permission::UserPermissions;
use scouter_drift::psi::PsiDrifter;
use scouter_settings::ScouterServerConfig;
use scouter_sql::sql::traits::{
    CustomMetricSqlLogic, LLMDriftSqlLogic, ProfileSqlLogic, SpcSqlLogic,
};
use scouter_sql::PostgresClient;
use scouter_types::{
    llm::PaginationResponse,
    psi::{BinnedPsiFeatureMetrics, PsiDriftProfile},
    spc::SpcDriftFeatures,
    BinnedMetrics, DriftType, LLMDriftRecordPaginationRequest, LLMDriftServerRecord, ServerRecords,
};
use scouter_types::{DriftRequest, GetProfileRequest, ScouterResponse, ScouterServerError};
use sqlx::{Pool, Postgres};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use tracing::{debug, error, instrument};

#[instrument(skip_all)]
pub async fn get_spc_drift(
    State(data): State<Arc<AppState>>,
    Extension(perms): Extension<UserPermissions>,
    Query(params): Query<DriftRequest>,
) -> Result<Json<SpcDriftFeatures>, (StatusCode, Json<ScouterServerError>)> {
    // validate time window

    debug!("Querying drift records: {:?}", params);

    if !perms.has_read_permission(&params.space) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ScouterServerError::permission_denied()),
        ));
    }

    let query_result = PostgresClient::get_binned_spc_drift_records(
        &data.db_pool,
        &params,
        &data.config.database_settings.retention_period,
        &data.config.storage_settings,
    )
    .await;

    match query_result {
        Ok(result) => Ok(Json(result)),
        Err(e) => {
            error!("Failed to query drift records: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::query_records_error(e)),
            ))
        }
    }
}

/// Common method used in both the get_psi_drift and get_psi_viz_drift routes
#[instrument(skip_all)]
async fn get_binned_psi_feature_metrics(
    params: &DriftRequest,
    db_pool: &Pool<Postgres>,
    config: &Arc<ScouterServerConfig>,
) -> Result<BinnedPsiFeatureMetrics, ServerError> {
    debug!("Querying drift records: {:?}", params);

    let profile_request = GetProfileRequest {
        name: params.name.clone(),
        space: params.space.clone(),
        version: params.version.clone(),
        drift_type: DriftType::Psi,
    };

    let value = PostgresClient::get_drift_profile(db_pool, &profile_request)
        .await
        .inspect_err(|e| {
            error!("Failed to get drift profile: {:?}", e);
        })?;

    let profile: PsiDriftProfile = match value {
        Some(profile) => serde_json::from_value(profile).unwrap(),
        None => {
            return Err(ServerError::NoProfileFoundError);
        }
    };

    let drifter = PsiDrifter::new(profile);
    Ok(drifter
        .get_binned_drift_map(
            params,
            db_pool,
            &config.database_settings.retention_period,
            &config.storage_settings,
        )
        .await
        .inspect_err(|e| {
            error!("Failed to get binned drift map: {:?}", e);
        })?)
}

/// This route is used to get the drift data for the PSI visualization
///
/// The route will both psi calculations for each feature and time interval as well as overall bin proportions
#[instrument(skip_all)]
pub async fn get_psi_drift(
    State(data): State<Arc<AppState>>,
    Query(params): Query<DriftRequest>,
    Extension(perms): Extension<UserPermissions>,
) -> Result<Json<BinnedPsiFeatureMetrics>, (StatusCode, Json<ScouterServerError>)> {
    //1. check for permissions
    if !perms.has_read_permission(&params.space) {
        error!("User does not have read permission for drift data");
        // list user permissions
        return Err((
            StatusCode::FORBIDDEN,
            Json(ScouterServerError::permission_denied()),
        ));
    }
    // validate time window
    debug!("Querying drift records: {:?}", params);
    let feature_metrics =
        get_binned_psi_feature_metrics(&params, &data.db_pool, &data.config).await;

    match feature_metrics {
        Ok(feature_metrics) => Ok(Json(feature_metrics)),
        Err(e) => {
            error!("Failed to query drift records: {:?}", e);

            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::query_records_error(e)),
            ))
        }
    }
}

#[instrument(skip_all)]
pub async fn get_custom_drift(
    State(data): State<Arc<AppState>>,
    Query(params): Query<DriftRequest>,
    Extension(perms): Extension<UserPermissions>,
) -> Result<Json<BinnedMetrics>, (StatusCode, Json<ScouterServerError>)> {
    // validate time window

    if !perms.has_read_permission(&params.space) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ScouterServerError::permission_denied()),
        ));
    }

    let metrics = PostgresClient::get_binned_custom_drift_records(
        &data.db_pool,
        &params,
        &data.config.database_settings.retention_period,
        &data.config.storage_settings,
    )
    .await;

    match metrics {
        Ok(metrics) => Ok(Json(metrics)),
        Err(e) => {
            error!("Failed to query drift records: {:?}", e);

            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::query_records_error(e)),
            ))
        }
    }
}

/// This route is used to get the latest LLM drift records by page
#[instrument(skip_all)]
pub async fn get_llm_drift_records(
    State(data): State<Arc<AppState>>,
    Query(params): Query<LLMDriftRecordPaginationRequest>,
    Extension(perms): Extension<UserPermissions>,
) -> Result<Json<PaginationResponse<LLMDriftServerRecord>>, (StatusCode, Json<ScouterServerError>)>
{
    // validate time window

    if !perms.has_read_permission(&params.service_info.space) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ScouterServerError::permission_denied()),
        ));
    }

    let metrics = PostgresClient::get_llm_drift_records_pagination(
        &data.db_pool,
        &params.service_info,
        params.status,
        params.pagination,
    )
    .await;

    match metrics {
        Ok(metrics) => Ok(Json(metrics)),
        Err(e) => {
            error!("Failed to query drift records: {:?}", e);

            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::query_records_error(e)),
            ))
        }
    }
}

#[instrument(skip_all)]
pub async fn get_llm_drift_metrics(
    State(data): State<Arc<AppState>>,
    Query(params): Query<DriftRequest>,
    Extension(perms): Extension<UserPermissions>,
) -> Result<Json<BinnedMetrics>, (StatusCode, Json<ScouterServerError>)> {
    // validate time window

    if !perms.has_read_permission(&params.space) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ScouterServerError::permission_denied()),
        ));
    }

    let metrics = PostgresClient::get_binned_llm_metric_values(
        &data.db_pool,
        &params,
        &data.config.database_settings.retention_period,
        &data.config.storage_settings,
    )
    .await;

    match metrics {
        Ok(metrics) => Ok(Json(metrics)),
        Err(e) => {
            error!("Failed to query drift records: {:?}", e);

            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::query_records_error(e)),
            ))
        }
    }
}

#[instrument(skip_all)]
pub async fn insert_drift(
    State(data): State<Arc<AppState>>,
    Extension(perms): Extension<UserPermissions>,
    Json(body): Json<ServerRecords>,
) -> Result<Json<ScouterResponse>, (StatusCode, Json<ScouterServerError>)> {
    if !perms.has_write_permission(&body.space()) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ScouterServerError::permission_denied()),
        ));
    }

    match data.http_consumer_tx.send_async(body).await {
        Ok(_) => Ok(Json(ScouterResponse {
            status: "success".to_string(),
            message: "Drift records queued for processing".to_string(),
        })),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ScouterServerError::new(format!(
                "Failed to enqueue drift records: {e:?}"
            ))),
        )),
    }
}

pub async fn get_drift_router(prefix: &str) -> Result<Router<Arc<AppState>>> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        Router::new()
            .route(&format!("{prefix}/drift"), post(insert_drift))
            .route(&format!("{prefix}/drift/spc"), get(get_spc_drift))
            .route(&format!("{prefix}/drift/custom"), get(get_custom_drift))
            .route(&format!("{prefix}/drift/psi"), get(get_psi_drift))
            .route(&format!("{prefix}/drift/llm"), get(get_llm_drift_metrics))
            .route(
                &format!("{prefix}/drift/llm/records"),
                get(get_llm_drift_records),
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
