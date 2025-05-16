use crate::sql::query::Queries;
use crate::sql::schema::TaskRequest;

use chrono::Utc;
use cron::Schedule;

use crate::sql::error::SqlError;
use async_trait::async_trait;
use scouter_types::{DriftProfile, GetProfileRequest, ProfileStatusRequest, ServiceInfo};
use serde_json::Value;
use sqlx::{postgres::PgQueryResult, Pool, Postgres, Row, Transaction};
use std::result::Result::Ok;
use std::str::FromStr;
use tracing::{error, instrument};

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
    #[instrument(skip_all)]
    async fn insert_drift_profile(
        pool: &Pool<Postgres>,
        drift_profile: &DriftProfile,
    ) -> Result<PgQueryResult, SqlError> {
        let query = Queries::InsertDriftProfile.get_query();
        let base_args = drift_profile.get_base_args();

        let current_time = Utc::now();

        let schedule = Schedule::from_str(&base_args.schedule)?;

        let next_run = match schedule.upcoming(Utc).take(1).next() {
            Some(next_run) => next_run,
            None => {
                return Err(SqlError::GetNextRunError);
            }
        };

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
            .map_err(SqlError::SqlxError)
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
            .map_err(SqlError::SqlxError)
    }

    /// Get a drift profile from the database
    ///
    /// # Arguments
    ///
    /// * `request` - The request to get the profile for
    ///
    /// # Returns
    async fn get_drift_profile(
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
            .map_err(SqlError::SqlxError)?;

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
        sqlx::query_as(&query.sql)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(SqlError::SqlxError)
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
    #[instrument(skip_all)]
    async fn update_drift_profile_run_dates(
        transaction: &mut Transaction<'_, Postgres>,
        service_info: &ServiceInfo,
        schedule: &str,
    ) -> Result<(), SqlError> {
        let query = Queries::UpdateDriftProfileRunDates.get_query();

        let schedule = Schedule::from_str(schedule)?;

        let next_run = match schedule.upcoming(Utc).take(1).next() {
            Some(next_run) => next_run,
            None => {
                return Err(SqlError::GetNextRunError);
            }
        };

        let query_result = sqlx::query(&query.sql)
            .bind(next_run)
            .bind(&service_info.name)
            .bind(&service_info.space)
            .bind(&service_info.version)
            .execute(&mut **transaction)
            .await
            .map_err(SqlError::SqlxError);

        match query_result {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    async fn update_drift_profile_status(
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
            .await
            .map_err(SqlError::SqlxError);

        match query_result {
            Ok(_) => {
                if params.deactivate_others {
                    let query = Queries::DeactivateDriftProfiles.get_query();

                    let query_result = sqlx::query(&query.sql)
                        .bind(&params.name)
                        .bind(&params.space)
                        .bind(&params.version)
                        .bind(params.drift_type.as_ref().map(|t| t.to_string()))
                        .execute(pool)
                        .await
                        .map_err(SqlError::SqlxError);

                    match query_result {
                        Ok(_) => Ok(()),
                        Err(e) => {
                            error!("Failed to deactivate other drift profiles: {:?}", e);
                            Err(e)
                        }
                    }
                } else {
                    Ok(())
                }
            }
            Err(e) => {
                error!("Failed to update drift profile status: {:?}", e);
                Err(e)
            }
        }
    }
}
