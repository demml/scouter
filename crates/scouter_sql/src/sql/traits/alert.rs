use crate::sql::query::Queries;
use crate::sql::schema::{AlertWrapper, UpdateAlertResult};

use scouter_types::contracts::{DriftAlertRequest, ServiceInfo, UpdateAlertStatus};

use crate::sql::error::SqlError;
use scouter_types::alert::Alert;
use scouter_types::DriftType;

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
    /// * `name` - The name of the service to insert the alert for
    /// * `space` - The name of the space to insert the alert for
    /// * `version` - The version of the service to insert the alert for
    /// * `alert` - The alert to insert into the database
    ///
    async fn insert_drift_alert(
        pool: &Pool<Postgres>,
        service_info: &ServiceInfo,
        feature: &str,
        alert: &BTreeMap<String, String>,
        drift_type: &DriftType,
    ) -> Result<PgQueryResult, SqlError> {
        let query = Queries::InsertDriftAlert.get_query();

        let query_result = sqlx::query(&query.sql)
            .bind(&service_info.name)
            .bind(&service_info.space)
            .bind(&service_info.version)
            .bind(feature)
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
            query.push_str(&format!(" LIMIT {}", limit));
        }

        // convert limit timestamp to string if it exists, leave as None if not

        let result: Result<Vec<AlertWrapper>, SqlError> = sqlx::query_as(&query)
            .bind(&params.version)
            .bind(&params.name)
            .bind(&params.space)
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

        match result {
            Ok(result) => Ok(result),
            Err(e) => Err(e),
        }
    }
}
