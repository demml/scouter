use crate::api::state::AppState;
use anyhow::{Context, Result};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Extension, Json, Router,
};
use scouter_auth::permission::UserPermissions;
use scouter_sql::sql::traits::GenAIDriftSqlLogic;
use scouter_sql::PostgresClient;
use scouter_types::ScouterServerError;
use scouter_types::{
    GenAIEvalRecordPaginationRequest, GenAIEvalRecordPaginationResponse, GenAIEvalTaskRequest,
    GenAIEvalTaskResult, GenAIEvalWorkflowPaginationResponse,
};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use tracing::{error, instrument};

/// This route is used to get the latest GenAI drift records by page
#[instrument(skip_all)]
pub async fn query_genai_eval_records(
    State(data): State<Arc<AppState>>,
    Extension(perms): Extension<UserPermissions>,
    Json(params): Json<GenAIEvalRecordPaginationRequest>,
) -> Result<Json<GenAIEvalRecordPaginationResponse>, (StatusCode, Json<ScouterServerError>)> {
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

    let metrics =
        PostgresClient::get_paginated_genai_eval_records(&data.db_pool, &params, &entity_id).await;

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
pub async fn query_genai_eval_workflow(
    State(data): State<Arc<AppState>>,
    Json(params): Json<GenAIEvalRecordPaginationRequest>,
) -> Result<Json<GenAIEvalWorkflowPaginationResponse>, (StatusCode, Json<ScouterServerError>)> {
    // validate time window

    let entity_id = data
        .get_entity_id_for_request(&params.service_info.uid)
        .await?;

    let metrics = PostgresClient::get_paginated_genai_eval_workflow_records(
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
pub async fn get_genai_tasks(
    State(data): State<Arc<AppState>>,
    Query(params): Query<GenAIEvalTaskRequest>,
) -> Result<Json<Vec<GenAIEvalTaskResult>>, (StatusCode, Json<ScouterServerError>)> {
    // validate time window

    let tasks = PostgresClient::get_genai_eval_task(&data.db_pool, &params.record_uid).await;

    match tasks {
        Ok(tasks) => Ok(Json(tasks)),
        Err(e) => {
            error!("Failed to query genai eval task metrics: {:?}", e);

            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::query_records_error(e)),
            ))
        }
    }
}

pub async fn get_genai_router(prefix: &str) -> Result<Router<Arc<AppState>>> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        Router::new()
            .route(&format!("{prefix}/genai/task"), get(get_genai_tasks))
            .route(
                &format!("{prefix}/genai/page/workflow"),
                post(query_genai_eval_workflow),
            )
            .route(
                &format!("{prefix}/genai/page/record"),
                post(query_genai_eval_records),
            )
    }));

    match result {
        Ok(router) => Ok(router),
        Err(_) => {
            // panic
            Err(anyhow::anyhow!("Failed to create genai router"))
                .context("Panic occurred while creating the router")
        }
    }
}
