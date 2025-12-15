use crate::api::state::AppState;

use anyhow::{Context, Result};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use scouter_sql::sql::traits::TagSqlLogic;
use scouter_sql::PostgresClient;
use scouter_types::{
    contracts::ScouterServerError, EntityIdTagsResponse, InsertTagsRequest, ScouterResponse,
    TagsResponse,
};
use scouter_types::{EntityIdTagsRequest, TagsRequest};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use tracing::{debug, error, instrument};

#[instrument(skip_all)]
pub async fn get_tags(
    State(data): State<Arc<AppState>>,
    Query(params): Query<TagsRequest>,
) -> Result<Json<TagsResponse>, (StatusCode, Json<ScouterServerError>)> {
    let tags = PostgresClient::get_tags(&data.db_pool, &params.entity_type, &params.entity_id)
        .await
        .map_err(|e| {
            error!("Failed to query tags: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::query_tags_error(e)),
            )
        })?;

    Ok(Json(TagsResponse { tags }))
}

#[instrument(skip_all)]
pub async fn insert_tags(
    State(data): State<Arc<AppState>>,
    Json(body): Json<InsertTagsRequest>,
) -> Result<Json<ScouterResponse>, (StatusCode, Json<ScouterServerError>)> {
    let tags = PostgresClient::insert_tag_batch(&data.db_pool, &body.tags)
        .await
        .map_err(|e| {
            error!("Failed to insert tags: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScouterServerError::insert_tags_error(e)),
            )
        })?;

    Ok(Json(ScouterResponse {
        status: "success".to_string(),
        message: format!("Inserted {} tags", tags.rows_affected()),
    }))
}

#[instrument(skip_all)]
pub async fn get_entity_id_from_tags(
    State(data): State<Arc<AppState>>,
    Query(params): Query<EntityIdTagsRequest>,
) -> Result<Json<EntityIdTagsResponse>, (StatusCode, Json<ScouterServerError>)> {
    debug!("Params: {:?}", params);
    let entity_id = PostgresClient::get_entity_id_by_tags(
        &data.db_pool,
        &params.entity_type,
        &params.tags,
        params.match_all,
    )
    .await
    .map_err(|e| {
        error!("Failed to get entity IDs by tags: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ScouterServerError::get_entity_id_by_tags_error(e)),
        )
    })?;

    Ok(Json(EntityIdTagsResponse { entity_id }))
}

#[instrument(skip_all)]
pub async fn get_tag_router(prefix: &str) -> Result<Router<Arc<AppState>>> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        Router::new()
            .route(&format!("{prefix}/tags"), get(get_tags).post(insert_tags))
            .route(
                &format!("{prefix}/tags/entity"),
                get(get_entity_id_from_tags),
            )
    }));

    match result {
        Ok(router) => Ok(router),
        Err(_) => {
            // panic
            Err(anyhow::anyhow!("Failed to create tag router"))
                .context("Panic occurred while creating the router")
        }
    }
}
