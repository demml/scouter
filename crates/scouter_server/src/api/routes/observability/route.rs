use scouter_types::{contracts::ObservabilityMetricRequest, ScouterServerError};

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

use std::sync::Arc;
use tracing::error;

use crate::api::state::AppState;
use anyhow::{Context, Result};
use axum::{routing::get, Router};
use scouter_sql::sql::traits::ObservabilitySqlLogic;
use scouter_sql::PostgresClient;
use std::panic::{catch_unwind, AssertUnwindSafe};

pub async fn get_observability_metrics(
    State(data): State<Arc<AppState>>,
    params: Query<ObservabilityMetricRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ScouterServerError>)> {
    let entity_id = data.get_entity_id_for_request(&params.uid).await?;
    let query_result =
        PostgresClient::get_binned_observability_metrics(&data.db_pool, &params, &entity_id).await;

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
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::new(format!(
                    "Failed to query observability metrics: {e:?}"
                ))),
            ))
        }
    }
}

pub async fn get_observability_router(prefix: &str) -> Result<Router<Arc<AppState>>> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        Router::new().route(
            &format!("{prefix}/observability/metrics"),
            get(get_observability_metrics),
        )
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
