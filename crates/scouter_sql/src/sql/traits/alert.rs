use crate::sql::query::Queries;
use crate::sql::schema::UpdateAlertResult;

use scouter_types::contracts::{DriftAlertRequest, UpdateAlertStatus};

use crate::sql::error::SqlError;
use scouter_types::alert::Alert;

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
    async fn get_drift_alerts(
        pool: &Pool<Postgres>,
        params: &DriftAlertRequest,
        entity_id: &i32,
    ) -> Result<Vec<Alert>, SqlError> {
        let mut query = Queries::GetDriftAlerts.get_query().to_string();

        if let Some(limit) = params.limit {
            query.push_str(&format!(" LIMIT {limit}"));
        }

        let result: Result<Vec<Alert>, SqlError> = sqlx::query_as(&query)
            .bind(entity_id)
            .bind(params.limit_datetime)
            .fetch_all(pool)
            .await
            .map_err(SqlError::SqlxError);

        result
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
