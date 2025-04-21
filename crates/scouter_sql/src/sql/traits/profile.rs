use crate::sql::query::Queries;
use crate::sql::schema::TaskRequest;

use chrono::Utc;
use cron::Schedule;
use scouter_contracts::{GetProfileRequest, ProfileStatusRequest, ServiceInfo};

use async_trait::async_trait;
use scouter_error::{SqlError, UtilError};
use scouter_types::DriftProfile;
use serde_json::Value;
use sqlx::{postgres::PgQueryResult, Pool, Postgres, Row, Transaction};
use std::result::Result::Ok;
use std::str::FromStr;
use tracing::error;

#[async_trait]
pub trait ProfileSqlLogic {
    /// Insert a drift profile into the database
    ///
    /// # Arguments
    ///
    /// * `drift_profile` - The drift profile to insert
    ///
    /// # Returns
    ///
    /// * `Result<PgQueryResult, SqlError>` - Result of the query
    async fn insert_drift_profile(
        &self,
        pool: &Pool<Postgres>,
        drift_profile: &DriftProfile,
    ) -> Result<PgQueryResult, SqlError> {
        let query = Queries::InsertDriftProfile.get_query();
        let base_args = drift_profile.get_base_args();

        let current_time = Utc::now();

        let schedule =
            Schedule::from_str(&base_args.schedule).map_err(UtilError::traced_parse_cron_error)?;

        let next_run = schedule
            .upcoming(Utc)
            .take(1)
            .next()
            .ok_or(SqlError::traced_get_next_run_error(&base_args.schedule))?;

        sqlx::query(&query.sql)
            .bind(base_args.name)
            .bind(base_args.space)
            .bind(base_args.version)
            .bind(base_args.scouter_version)
            .bind(drift_profile.to_value())
            .bind(base_args.drift_type.to_string())
            .bind(false)
            .bind(base_args.schedule)
            .bind(next_run)
            .bind(current_time)
            .execute(pool)
            .await
            .map_err(SqlError::traced_query_error)
    }

    /// Update a drift profile in the database
    ///
    /// # Arguments
    ///
    /// * `drift_profile` - The drift profile to update
    ///
    /// # Returns
    ///
    /// * `Result<PgQueryResult, SqlError>` - Result of the query
    async fn update_drift_profile(
        &self,
        pool: &Pool<Postgres>,
        drift_profile: &DriftProfile,
    ) -> Result<PgQueryResult, SqlError> {
        let query = Queries::UpdateDriftProfile.get_query();
        let base_args = drift_profile.get_base_args();

        sqlx::query(&query.sql)
            .bind(drift_profile.to_value())
            .bind(base_args.drift_type.to_string())
            .bind(base_args.name)
            .bind(base_args.space)
            .bind(base_args.version)
            .execute(pool)
            .await
            .map_err(SqlError::traced_query_error)
    }

    /// Get a drift profile from the database
    ///
    /// # Arguments
    ///
    /// * `request` - The request to get the profile for
    ///
    /// # Returns
    async fn get_drift_profile(
        &self,
        pool: &Pool<Postgres>,
        request: &GetProfileRequest,
    ) -> Result<Option<Value>, SqlError> {
        let query = Queries::GetDriftProfile.get_query();

        let result = sqlx::query(&query.sql)
            .bind(&request.name)
            .bind(&request.space)
            .bind(&request.version)
            .bind(request.drift_type.to_string())
            .fetch_optional(pool)
            .await
            .map_err(SqlError::traced_query_error)?;

        match result {
            Some(result) => {
                let profile: Value = result.get("profile");
                Ok(Some(profile))
            }
            None => Ok(None),
        }
    }

    async fn get_drift_profile_task(
        transaction: &mut Transaction<'_, Postgres>,
    ) -> Result<Option<TaskRequest>, SqlError> {
        let query = Queries::GetDriftTask.get_query();
        let result: Result<Option<TaskRequest>, sqlx::Error> = sqlx::query_as(&query.sql)
            .fetch_optional(&mut **transaction)
            .await;

        result.map_err(SqlError::traced_get_drift_task_error)
    }

    /// Update the drift profile run dates in the database
    ///
    /// # Arguments
    ///
    /// * `transaction` - The database transaction
    /// * `service_info` - The service info to update the run dates for
    /// * `schedule` - The schedule to update the run dates with
    ///
    /// # Returns
    ///
    /// * `Result<(), SqlError>` - Result of the query
    async fn update_drift_profile_run_dates(
        transaction: &mut Transaction<'_, Postgres>,
        service_info: &ServiceInfo,
        schedule: &str,
    ) -> Result<(), SqlError> {
        let query = Queries::UpdateDriftProfileRunDates.get_query();

        let schedule = Schedule::from_str(schedule).map_err(UtilError::traced_parse_cron_error)?;

        let next_run = schedule
            .upcoming(Utc)
            .take(1)
            .next()
            .ok_or(SqlError::traced_get_next_run_error(schedule))?;

        let query_result = sqlx::query(&query.sql)
            .bind(next_run)
            .bind(&service_info.name)
            .bind(&service_info.space)
            .bind(&service_info.version)
            .execute(&mut **transaction)
            .await;

        match query_result {
            Ok(_) => Ok(()),
            Err(e) => Err(SqlError::traced_update_drift_profile_error(e)),
        }
    }

    async fn update_drift_profile_status(
        &self,
        pool: &Pool<Postgres>,
        params: &ProfileStatusRequest,
    ) -> Result<(), SqlError> {
        let query = Queries::UpdateDriftProfileStatus.get_query();

        // convert drift_type to string or None
        let query_result = sqlx::query(&query.sql)
            .bind(params.active)
            .bind(&params.name)
            .bind(&params.space)
            .bind(&params.version)
            .bind(params.drift_type.as_ref().map(|t| t.to_string()))
            .execute(pool)
            .await;

        match query_result {
            Ok(_) => Ok(()),
            Err(e) => {
                error!("Failed to update drift profile status: {:?}", e);
                Err(SqlError::traced_update_drift_profile_error(e))
            }
        }
    }
}
