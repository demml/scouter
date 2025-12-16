use crate::sql::query::Queries;
use crate::sql::schema::UpdateAlertResult;

use scouter_types::contracts::{
    DriftAlertPaginationRequest, DriftAlertPaginationResponse, UpdateAlertStatus,
};

use crate::sql::error::SqlError;
use scouter_types::{alert::Alert, RecordCursor};

use sqlx::{postgres::PgQueryResult, Pool, Postgres};
use std::collections::BTreeMap;
use std::result::Result::Ok;

use async_trait::async_trait;

#[async_trait]
pub trait AlertSqlLogic {
    /// Inserts a drift alert into the database
    ///
    /// # Arguments
    ///
    /// * `task_info` - The drift task info containing entity_id
    /// * `entity_name` - The name of the entity
    /// * `alert` - The alert to insert into the database
    /// * `drift_type` - The type of drift alert
    ///
    async fn insert_drift_alert(
        pool: &Pool<Postgres>,
        entity_id: &i32,
        entity_name: &str,
        alert: &BTreeMap<String, String>,
    ) -> Result<PgQueryResult, SqlError> {
        let query = Queries::InsertDriftAlert.get_query();

        let query_result = sqlx::query(query)
            .bind(entity_id)
            .bind(entity_name)
            .bind(serde_json::to_value(alert).unwrap())
            .execute(pool)
            .await?;

        Ok(query_result)
    }

    /// Get drift alerts from the database
    ///
    /// # Arguments
    ///
    /// * `params` - The drift alert request parameters
    /// * `id` - The entity ID to filter alerts
    ///
    /// # Returns
    ///
    /// * `Result<Vec<Alert>, SqlError>` - Result of the query
    async fn get_paginated_drift_alerts(
        pool: &Pool<Postgres>,
        params: &DriftAlertPaginationRequest,
        entity_id: &i32,
    ) -> Result<DriftAlertPaginationResponse, SqlError> {
        let query = Queries::GetPaginatedDriftAlerts.get_query();
        let limit = params.limit.unwrap_or(50);
        let direction = params.direction.as_deref().unwrap_or("next");

        let mut items: Vec<Alert> = sqlx::query_as(query)
            .bind(entity_id) // $1: entity_id
            .bind(params.active) // $2: active filter
            .bind(params.cursor_created_at) // $3: cursor created_at
            .bind(direction) // $4: direction
            .bind(params.cursor_id) // $5: cursor id
            .bind(limit) // $6: limit
            .fetch_all(pool)
            .await
            .map_err(SqlError::SqlxError)?;

        let has_more = items.len() > limit as usize;

        if has_more {
            items.pop();
        }

        let (has_next, next_cursor, has_previous, previous_cursor) = match direction {
            "previous" => {
                // Backward pagination - reverse since we fetched in ASC order
                items.reverse();

                let previous_cursor = if has_more {
                    items.first().map(|first| RecordCursor {
                        created_at: first.created_at,
                        id: first.id as i64,
                    })
                } else {
                    None
                };

                let next_cursor = items.last().map(|last| RecordCursor {
                    created_at: last.created_at,
                    id: last.id as i64,
                });

                (
                    params.cursor_created_at.is_some(), // has_next (we came from somewhere)
                    next_cursor,
                    has_more, // has_previous (more items before)
                    previous_cursor,
                )
            }
            _ => {
                // Forward pagination (default)
                let next_cursor = if has_more {
                    items.last().map(|last| RecordCursor {
                        created_at: last.created_at,
                        id: last.id as i64,
                    })
                } else {
                    None
                };

                let previous_cursor = items.first().map(|first| RecordCursor {
                    created_at: first.created_at,
                    id: first.id as i64,
                });

                (
                    has_more, // has_next (more items after)
                    next_cursor,
                    params.cursor_created_at.is_some(), // has_previous (we came from somewhere)
                    previous_cursor,
                )
            }
        };

        Ok(DriftAlertPaginationResponse {
            items,
            has_next,
            next_cursor,
            has_previous,
            previous_cursor,
        })
    }

    /// Update drift alert status in the database
    ////
    /// # Arguments
    ///// * `params` - The update alert status parameters
    /// # Returns
    //// * `Result<UpdateAlertResult, SqlError>` - Result of the update operation
    async fn update_drift_alert_status(
        pool: &Pool<Postgres>,
        params: &UpdateAlertStatus,
    ) -> Result<UpdateAlertResult, SqlError> {
        let query = Queries::UpdateAlertStatus.get_query();

        let result: Result<UpdateAlertResult, SqlError> = sqlx::query_as(query)
            .bind(params.id)
            .bind(params.active)
            .fetch_one(pool)
            .await
            .map_err(SqlError::SqlxError);

        result
    }
}
