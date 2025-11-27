use crate::sql::query::Queries;
use crate::sql::schema::{AlertWrapper, UpdateAlertResult};

use scouter_types::contracts::{DriftAlertRequest, UpdateAlertStatus};

use crate::sql::error::SqlError;
use scouter_types::alert::Alert;
use scouter_types::{DriftTaskInfo, DriftType};

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
        task_info: &DriftTaskInfo,
        entity_name: &str,
        alert: &BTreeMap<String, String>,
        drift_type: &DriftType,
    ) -> Result<PgQueryResult, SqlError> {
        let query = Queries::InsertDriftAlert.get_query();

        let query_result = sqlx::query(&query.sql)
            .bind(task_info.id)
            .bind(entity_name)
            .bind(serde_json::to_value(alert).unwrap())
            .bind(drift_type.to_string())
            .execute(pool)
            .await?;

        Ok(query_result)
    }

    /// Get drift alerts from the database
    ///
    /// # Arguments
    ///
    /// * `params` - The drift alert request parameters
    ///
    /// # Returns
    ///
    /// * `Result<Vec<Alert>, SqlError>` - Result of the query
    async fn get_drift_alerts(
        pool: &Pool<Postgres>,
        params: &DriftAlertRequest,
    ) -> Result<Vec<Alert>, SqlError> {
        let mut query = Queries::GetDriftAlerts.get_query().sql;

        if params.active.unwrap_or(false) {
            query.push_str(" AND active = true");
        }

        query.push_str(" ORDER BY created_at DESC");

        if let Some(limit) = params.limit {
            query.push_str(&format!(" LIMIT {limit}"));
        }

        // convert limit timestamp to string if it exists, leave as None if not

        let result: Result<Vec<AlertWrapper>, SqlError> = sqlx::query_as(&query)
            .bind(params.entity_id)
            .bind(params.limit_datetime)
            .fetch_all(pool)
            .await
            .map_err(SqlError::SqlxError);

        result.map(|result| result.into_iter().map(|wrapper| wrapper.0).collect())
    }

    async fn update_drift_alert_status(
        pool: &Pool<Postgres>,
        params: &UpdateAlertStatus,
    ) -> Result<UpdateAlertResult, SqlError> {
        let query = Queries::UpdateAlertStatus.get_query();

        let result: Result<UpdateAlertResult, SqlError> = sqlx::query_as(&query.sql)
            .bind(params.id)
            .bind(params.active)
            .fetch_one(pool)
            .await
            .map_err(SqlError::SqlxError);

        result
    }
}
