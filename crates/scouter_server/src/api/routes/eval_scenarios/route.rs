use crate::api::state::AppState;
use anyhow::Result;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use scouter_evaluate::scenario::EvalScenarios;
use scouter_types::contracts::ScouterServerError;
use serde::Deserialize;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use tracing::{error, instrument};

#[derive(Deserialize)]
pub struct CollectionIdQuery {
    pub collection_id: String,
}

#[instrument(skip_all)]
pub async fn get_eval_scenarios(
    State(data): State<Arc<AppState>>,
    Query(params): Query<CollectionIdQuery>,
) -> Result<Json<EvalScenarios>, (StatusCode, Json<ScouterServerError>)> {
    let records = data
        .eval_scenario_service
        .get_scenarios(&params.collection_id)
        .await
        .map_err(|e| {
            error!(error = ?e, "Failed to get eval scenarios");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::new(
                    "Failed to get eval scenarios".to_string(),
                )),
            )
        })?;

    if records.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ScouterServerError::new(format!(
                "No scenarios found for collection_id: {}",
                params.collection_id
            ))),
        ));
    }

    let collection_id = params.collection_id.clone();
    let scenarios = records
        .into_iter()
        .map(|r| serde_json::from_str(&r.scenario_json))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            error!(error = %e, "Failed to deserialize scenario JSON");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::new(
                    "Failed to deserialize scenario".to_string(),
                )),
            )
        })?;

    let mut eval_scenarios = EvalScenarios::new(scenarios);
    eval_scenarios.collection_id = collection_id;

    Ok(Json(eval_scenarios))
}

pub fn get_eval_scenario_router(prefix: &str) -> Router<Arc<AppState>> {
    catch_unwind(AssertUnwindSafe(|| {
        Router::new().route(&format!("{prefix}/eval/scenarios"), get(get_eval_scenarios))
    }))
    .expect("Failed to create eval scenario router")
}
