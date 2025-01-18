use crate::api::state::AppState;
use anyhow::{Context, Result};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::TimeDelta;
use scouter_contracts::{DriftRequest, GetProfileRequest, ServiceInfo};
use scouter_drift::psi::PsiDrifter;
use scouter_error::ScouterError;
use scouter_sql::PostgresClient;
use scouter_types::{
    psi::{BinnedPsiFeatureMetrics, PsiDriftProfile, PsiDriftViz},
    DriftType, RecordType, ServerRecords, ToDriftRecords,
};
use serde_json::json;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use tracing::error;

pub async fn get_spc_drift(
    State(data): State<Arc<AppState>>,
    Query(params): Query<DriftRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    // validate time window

    let query_result = &data.db.get_binned_spc_drift_records(&params).await;

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

/// Common method used in both the get_psi_drift and get_psi_viz_drift routes
async fn get_binned_psi_feature_metrics(
    params: &DriftRequest,
    db: &PostgresClient,
) -> Result<(BinnedPsiFeatureMetrics, PsiDriftProfile), ScouterError> {
    let profile_request = GetProfileRequest {
        name: params.name.clone(),
        repository: params.repository.clone(),
        version: params.version.clone(),
        drift_type: DriftType::Psi,
    };

    let value = db.get_drift_profile(&profile_request).await?;

    let profile: PsiDriftProfile = match value {
        Some(profile) => serde_json::from_value(profile).unwrap(),
        None => {
            return Err(ScouterError::Error("Failed to load profile".to_string()));
        }
    };

    let drifter = PsiDrifter::new(profile.clone());
    Ok((drifter.get_binned_drift_map(params, db).await?, profile))
}

pub async fn get_psi_drift(
    State(data): State<Arc<AppState>>,
    Query(params): Query<DriftRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    // validate time window

    let feature_metrics = get_binned_psi_feature_metrics(&params, &data.db).await;

    match feature_metrics {
        Ok((metrics, _)) => {
            let json_response = serde_json::json!(metrics);
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

/// This route is used to get the drift data for the PSI visualization
///
/// The route will both psi calculations for each feature and time interval as well as overall bin proportions
pub async fn get_psi_viz_drift(
    State(data): State<Arc<AppState>>,
    Query(params): Query<DriftRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    // validate time window

    let feature_metrics = get_binned_psi_feature_metrics(&params, &data.db).await;

    let (feature_metrics, profile) = match feature_metrics {
        Ok((metrics, profile)) => (metrics, profile),
        Err(e) => {
            error!("Failed to query drift records: {:?}", e);
            let json_response = json!({
                "status": "error",
                "message": format!("{:?}", e)
            });
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json_response)));
        }
    };

    let service_info = ServiceInfo {
        name: params.name.clone(),
        repository: params.repository.clone(),
        version: params.version.clone(),
    };

    let minutes = params.time_window.to_minutes() as i64;
    let limit_datetime = chrono::Utc::now().naive_utc() - TimeDelta::minutes(minutes);
    let bin_proportions = data
        .db
        .get_feature_bin_proportions(
            &service_info,
            &limit_datetime,
            &profile.config.alert_config.features_to_monitor,
        )
        .await;

    match bin_proportions {
        Ok(bin_proportions) => {
            let json_response = json!(PsiDriftViz {
                feature_metrics,
                bin_proportions,
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

async fn insert_spc_drift(
    records: &ServerRecords,
    db: &PostgresClient,
) -> Result<(), ScouterError> {
    let records = records.to_spc_drift_records()?;

    for record in records {
        let _ = db.insert_spc_drift_record(&record).await?;
    }

    Ok(())
}

async fn insert_psi_drift(
    records: &ServerRecords,
    db: &PostgresClient,
) -> Result<(), ScouterError> {
    let records = records.to_psi_drift_records()?;

    for record in records {
        let _ = db.insert_bin_counts(&record).await?;
    }

    Ok(())
}

async fn insert_custom_drift(
    records: &ServerRecords,
    db: &PostgresClient,
) -> Result<(), ScouterError> {
    let records = records.to_custom_metric_drift_records()?;

    for record in records {
        let _ = db.insert_custom_metric_value(&record).await?;
    }

    Ok(())
}
pub async fn insert_drift(
    State(data): State<Arc<AppState>>,
    Json(body): Json<ServerRecords>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let inserted = match body.record_type {
        RecordType::Spc => insert_spc_drift(&body, &data.db).await,
        RecordType::Psi => insert_psi_drift(&body, &data.db).await,
        RecordType::Custom => insert_custom_drift(&body, &data.db).await,
        _ => Err(ScouterError::Error("Invalid record type".to_string())),
    };

    match inserted {
        Ok(_) => {
            let json_response = json!({
                "status": "success",
                "message": "Record inserted successfully"
            });
            Ok(Json(json_response))
        }
        Err(e) => {
            error!("Failed to insert drift record: {:?}", e);
            let json_response = json!({
                "status": "error",
                "message": format!("{:?}", e)
            });
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json_response)))
        }
    }
}

pub async fn get_drift_router(prefix: &str) -> Result<Router<Arc<AppState>>> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        Router::new()
            .route(&format!("{}/drift", prefix), post(insert_drift))
            .route(&format!("{}/drift/spc", prefix), get(get_spc_drift))
            .route(&format!("{}/drift/psi", prefix), get(get_psi_drift))
            .route(&format!("{}/drift/psi/viz", prefix), get(get_psi_viz_drift))
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
