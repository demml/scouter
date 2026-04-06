use crate::api::state::AppState;
use anyhow::{Context, Result};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Extension, Json, Router,
};
use scouter_auth::permission::UserPermissions;
use scouter_sql::sql::traits::AgentDriftSqlLogic;
use scouter_sql::PostgresClient;
use scouter_types::{
    AgentEvalTaskRequest, AgentEvalWorkflowPaginationResponse, EvalRecordPaginationRequest,
    EvalRecordPaginationResponse,
};
use scouter_types::{AgentEvalTaskResponse, ScouterServerError};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use tracing::{debug, error, instrument};

/// This route is used to get the latest GenAI drift records by page
#[instrument(skip_all)]
pub async fn query_agent_eval_records(
    State(data): State<Arc<AppState>>,
    Extension(perms): Extension<UserPermissions>,
    Json(params): Json<EvalRecordPaginationRequest>,
) -> Result<Json<EvalRecordPaginationResponse>, (StatusCode, Json<ScouterServerError>)> {
    // validate time window

    if !perms.has_read_permission(&params.service_info.space) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ScouterServerError::permission_denied()),
        ));
    }

    let entity_id = data
        .get_entity_id_for_request(&params.service_info.uid)
        .await?;

    debug!(
        "Querying agent eval records for entity_id and request: {} {:?}",
        entity_id, params
    );

    let metrics =
        PostgresClient::get_paginated_agent_eval_records(&data.db_pool, &params, &entity_id).await;

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

/// This route is used to get the latest GenAI drift records by page
#[instrument(skip_all)]
pub async fn query_agent_eval_workflow(
    State(data): State<Arc<AppState>>,
    Json(params): Json<EvalRecordPaginationRequest>,
) -> Result<Json<AgentEvalWorkflowPaginationResponse>, (StatusCode, Json<ScouterServerError>)> {
    // validate time window

    let entity_id = data
        .get_entity_id_for_request(&params.service_info.uid)
        .await?;

    let metrics = PostgresClient::get_paginated_agent_eval_workflow_records(
        &data.db_pool,
        &params,
        &entity_id,
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
pub async fn get_agent_tasks(
    State(data): State<Arc<AppState>>,
    Query(params): Query<AgentEvalTaskRequest>,
) -> Result<Json<AgentEvalTaskResponse>, (StatusCode, Json<ScouterServerError>)> {
    // validate time window

    let tasks = PostgresClient::get_agent_eval_task(&data.db_pool, &params.record_uid).await;

    match tasks {
        Ok(tasks) => Ok(Json(AgentEvalTaskResponse { tasks })),
        Err(e) => {
            error!("Failed to query agent eval task metrics: {:?}", e);

            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::query_records_error(e)),
            ))
        }
    }
}

pub async fn get_agent_router(prefix: &str) -> Result<Router<Arc<AppState>>> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        Router::new()
            .route(&format!("{prefix}/agent/task"), get(get_agent_tasks))
            .route(
                &format!("{prefix}/agent/page/workflow"),
                post(query_agent_eval_workflow),
            )
            .route(
                &format!("{prefix}/agent/page/record"),
                post(query_agent_eval_records),
            )
    }));

    match result {
        Ok(router) => Ok(router),
        Err(_) => {
            // panic
            Err(anyhow::anyhow!("Failed to create agent router"))
                .context("Panic occurred while creating the router")
        }
    }
}
