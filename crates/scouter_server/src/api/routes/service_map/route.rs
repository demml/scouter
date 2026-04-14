use crate::api::state::AppState;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::Utc;
use scouter_dataframe::{batches_to_edges, build_topology_sql, ServiceGraphEdge};
use scouter_dataframe::error::DatasetEngineError;
use scouter_types::contracts::ScouterServerError;
use std::sync::Arc;
use tracing::{error, info, instrument};
use uuid::Uuid;

#[derive(serde::Deserialize, utoipa::IntoParams)]
pub struct ServiceGraphQueryParams {
    /// Filter to a specific destination service. Omit to return all edges.
    service_name: Option<String>,
    since: Option<String>,
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct ServiceGraphResponse {
    pub edges: Vec<ServiceGraphEdge>,
    pub generated_at: String,
}

fn map_dataset_error(e: DatasetEngineError) -> (StatusCode, Json<ScouterServerError>) {
    let (status, msg) = match &e {
        DatasetEngineError::TableNotFound(_) => {
            error!("Service map table not found: {:?}", e);
            (StatusCode::NOT_FOUND, "Table not found".to_string())
        }
        DatasetEngineError::SqlValidationError(_) => {
            error!("Service map SQL validation error: {:?}", e);
            (StatusCode::BAD_REQUEST, "Invalid query parameters".to_string())
        }
        DatasetEngineError::ChannelClosed => {
            error!("Service map dataset channel closed: {:?}", e);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "Service temporarily unavailable".to_string(),
            )
        }
        _ => {
            error!("Service map query error: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            )
        }
    };
    (status, Json(ScouterServerError::new(msg)))
}

fn bad_request(msg: String) -> (StatusCode, Json<ScouterServerError>) {
    (StatusCode::BAD_REQUEST, Json(ScouterServerError::new(msg)))
}

fn internal_error(
    msg: &str,
    detail: impl std::fmt::Display,
) -> (StatusCode, Json<ScouterServerError>) {
    error!("{}: {}", msg, detail);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ScouterServerError::new("Internal server error".to_string())),
    )
}

#[utoipa::path(
    get,
    path = "/scouter/service/graph",
    params(ServiceGraphQueryParams),
    responses(
        (status = 200, description = "Service topology graph", body = ServiceGraphResponse),
        (status = 400, description = "Invalid params", body = ScouterServerError),
        (status = 404, description = "No data found", body = ScouterServerError),
        (status = 500, description = "Internal error", body = ScouterServerError),
    ),
    security(("bearer_token" = [])),
    tag = "service_map"
)]
#[instrument(skip_all)]
pub async fn get_service_graph(
    State(data): State<Arc<AppState>>,
    Query(params): Query<ServiceGraphQueryParams>,
) -> Result<Json<ServiceGraphResponse>, (StatusCode, Json<ScouterServerError>)> {
    info!(
        service_name = ?params.service_name,
        since = ?params.since,
        "service_graph_query"
    );

    let sql = build_topology_sql(params.service_name.as_deref(), params.since.as_deref())
        .map_err(bad_request)?;

    let query_id = Uuid::new_v4().to_string();
    let result = data
        .dataset_manager
        .execute_query(&sql, &query_id, 10_000)
        .await
        .map_err(map_dataset_error)?;

    let edges = tokio::task::spawn_blocking(move || batches_to_edges(&result.batches))
        .await
        .map_err(|e| internal_error("spawn error", e))?
        .map_err(|e| internal_error("Failed to parse graph edges", e))?;

    Ok(Json(ServiceGraphResponse {
        edges,
        generated_at: Utc::now().to_rfc3339(),
    }))
}

pub fn get_service_map_router(prefix: &str) -> Router<Arc<AppState>> {
    Router::new().route(
        &format!("{prefix}/service/graph"),
        get(get_service_graph),
    )
}
