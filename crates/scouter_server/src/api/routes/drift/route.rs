use crate::api::state::AppState;
use anyhow::{Context, Result};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Extension, Json, Router,
};
use scouter_auth::permission::UserPermissions;
use scouter_contracts::{DriftRequest, GetProfileRequest, ScouterResponse, ScouterServerError};
use scouter_drift::psi::PsiDrifter;
use scouter_error::ScouterError;
use scouter_settings::ScouterServerConfig;
use scouter_sql::sql::traits::{CustomMetricSqlLogic, ProfileSqlLogic, PsiSqlLogic, SpcSqlLogic};
use scouter_sql::PostgresClient;
use scouter_types::{
    custom::BinnedCustomMetrics,
    psi::{BinnedPsiFeatureMetrics, PsiDriftProfile},
    spc::SpcDriftFeatures,
    DriftType, RecordType, ServerRecords, ToDriftRecords,
};
use sqlx::{Pool, Postgres};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use tracing::{debug, error, instrument};

#[instrument(skip(data, params))]
pub async fn get_spc_drift(
    State(data): State<Arc<AppState>>,
    Extension(perms): Extension<UserPermissions>,
    Query(params): Query<DriftRequest>,
) -> Result<Json<SpcDriftFeatures>, (StatusCode, Json<ScouterServerError>)> {
    // validate time window

    debug!("Querying drift records: {:?}", params);

    if !perms.has_read_permission() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ScouterServerError::permission_denied()),
        ));
    }

    let query_result = PostgresClient::get_binned_spc_drift_records(&data.db_pool, &params).await;

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
) -> Result<BinnedPsiFeatureMetrics, ScouterError> {
    debug!("Querying drift records: {:?}", params);

    let profile_request = GetProfileRequest {
        name: params.name.clone(),
        space: params.space.clone(),
        version: params.version.clone(),
        drift_type: DriftType::Psi,
    };

    let value = PostgresClient::get_drift_profile(db_pool, &profile_request).await?;

    let profile: PsiDriftProfile = match value {
        Some(profile) => serde_json::from_value(profile).unwrap(),
        None => {
            return Err(ScouterError::Error("Failed to load profile".to_string()));
        }
    };

    let drifter = PsiDrifter::new(profile.clone());
    Ok(drifter
        .get_binned_drift_map(
            params,
            db_pool,
            &config.database_settings.retention_period,
            &config.storage_settings,
        )
        .await?)
}

/// This route is used to get the drift data for the PSI visualization
///
/// The route will both psi calculations for each feature and time interval as well as overall bin proportions
#[instrument(skip(data, params))]
pub async fn get_psi_drift(
    State(data): State<Arc<AppState>>,
    Query(params): Query<DriftRequest>,
    Extension(perms): Extension<UserPermissions>,
) -> Result<Json<BinnedPsiFeatureMetrics>, (StatusCode, Json<ScouterServerError>)> {
    //1. check for permissions
    if !perms.has_read_permission() {
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

#[instrument(skip(data, params))]
pub async fn get_custom_drift(
    State(data): State<Arc<AppState>>,
    Query(params): Query<DriftRequest>,
    Extension(perms): Extension<UserPermissions>,
) -> Result<Json<BinnedCustomMetrics>, (StatusCode, Json<ScouterServerError>)> {
    // validate time window

    if !perms.has_read_permission() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ScouterServerError::permission_denied()),
        ));
    }

    debug!("Querying drift records: {:?}", params);

    let metrics = PostgresClient::get_binned_custom_drift_records(&data.db_pool, &params).await;

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
async fn insert_spc_drift(
    records: &ServerRecords,
    db_pool: &Pool<Postgres>,
) -> Result<(), ScouterError> {
    let records = records.to_spc_drift_records()?;

    for record in records {
        PostgresClient::insert_spc_drift_record(db_pool, &record).await?;
    }

    Ok(())
}

#[instrument(skip_all)]
async fn insert_psi_drift(
    records: &ServerRecords,
    db_pool: &Pool<Postgres>,
) -> Result<(), ScouterError> {
    let records = records.to_psi_drift_records()?;

    for record in records {
        PostgresClient::insert_bin_counts(db_pool, &record).await?;
    }

    Ok(())
}

#[instrument(skip_all)]
async fn insert_custom_drift(
    records: &ServerRecords,
    db_pool: &Pool<Postgres>,
) -> Result<(), ScouterError> {
    let records = records.to_custom_metric_drift_records()?;

    for record in records {
        PostgresClient::insert_custom_metric_value(db_pool, &record).await?;
    }

    Ok(())
}

#[instrument(skip_all)]
pub async fn insert_drift(
    State(data): State<Arc<AppState>>,
    Extension(perms): Extension<UserPermissions>,
    Json(body): Json<ServerRecords>,
) -> Result<Json<ScouterResponse>, (StatusCode, Json<ScouterServerError>)> {
    debug!("Inserting drift record: {:?}", body);

    if !perms.has_write_permission(&body.space()) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ScouterServerError::permission_denied()),
        ));
    }

    let record_type = match body.record_type() {
        Ok(rt) => rt,
        Err(e) => {
            error!("Invalid record type: {:?}", e);
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ScouterServerError::new(format!(
                    "Invalid record type: {:?}",
                    e
                ))),
            ));
        }
    };

    let result = match record_type {
        RecordType::Spc => insert_spc_drift(&body, &data.db_pool).await,
        RecordType::Psi => insert_psi_drift(&body, &data.db_pool).await,
        RecordType::Custom => insert_custom_drift(&body, &data.db_pool).await,
        _ => Err(ScouterError::Error("Invalid record type".to_string())),
    };

    match result {
        Ok(_) => Ok(Json(ScouterResponse::new(
            "success".to_string(),
            "Drift record inserted successfully".to_string(),
        ))),
        Err(e) => {
            error!("Failed to insert drift record: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::new(format!(
                    "Failed to insert drift record: {:?}",
                    e
                ))),
            ))
        }
    }
}

pub async fn get_drift_router(prefix: &str) -> Result<Router<Arc<AppState>>> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        Router::new()
            .route(&format!("{}/drift", prefix), post(insert_drift))
            .route(&format!("{}/drift/spc", prefix), get(get_spc_drift))
            .route(&format!("{}/drift/custom", prefix), get(get_custom_drift))
            .route(&format!("{}/drift/psi", prefix), get(get_psi_drift))
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
