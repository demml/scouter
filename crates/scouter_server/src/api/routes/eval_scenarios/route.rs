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

#[derive(Deserialize, utoipa::IntoParams)]
pub struct CollectionIdQuery {
    pub collection_id: String,
}

#[utoipa::path(
    get,
    path = "/scouter/eval/scenarios",
    params(CollectionIdQuery),
    responses(
        (status = 200, description = "Evaluation scenarios for collection"),
        (status = 404, description = "No scenarios found", body = ScouterServerError),
        (status = 500, description = "Internal server error", body = ScouterServerError),
    ),
    tag = "eval",
    security(("bearer_token" = []))
)]
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
    let scenarios = tokio::task::spawn_blocking(move || {
        records
            .into_iter()
            .map(|r| serde_json::from_str(&r.scenario_json))
            .collect::<Result<Vec<_>, _>>()
    })
    .await
    .map_err(|e| {
        error!(error = %e, "spawn_blocking join error deserializing scenarios");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ScouterServerError::new(
                "Failed to deserialize scenario".to_string(),
            )),
        )
    })?
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
