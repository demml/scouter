use crate::sql::query::Queries;
use crate::sql::schema::TaskRequest;

use crate::sql::error::SqlError;
use crate::sql::schema::VersionResult;
use async_trait::async_trait;
use chrono::Utc;
use cron::Schedule;
use scouter_semver::{VersionParser, VersionValidator};
use scouter_types::{
    DriftProfile, DriftTaskInfo, GetProfileRequest, ProfileArgs, ProfileStatusRequest,
};
use semver::Version;
use serde_json::Value;
use sqlx::{postgres::PgQueryResult, Pool, Postgres, Row};
use std::result::Result::Ok;
use std::str::FromStr;
use tracing::{error, instrument};

//
//let versions = self
//                .sql_client
//                .get_versions(&self.table_name, space, name, version.clone())
//                .await?;
//
//            // if no versions exist, return the default version
//            if versions.is_empty() {
//                return match &version {
//                    Some(version_str) => Ok(VersionValidator::clean_version(version_str)?),
//                    None => Ok(Version::new(0, 1, 0)),
//                };
//            }
//
//            let base_version = versions.first().unwrap().to_string();
//
//            let args = VersionArgs {
//                version: base_version,
//                version_type,
//                pre: pre_tag,
//                build: build_tag,
//            };
//
//            Ok(VersionValidator::bump_version(&args)?)
//
/// Add bounds for version
pub fn add_version_bounds(builder: &mut String, version: &str) -> Result<(), SqlError> {
    let version_bounds = VersionParser::get_version_to_search(version)?;

    // construct lower bound (already validated)
    builder.push_str(
        format!(
            " AND (major >= {} AND minor >= {} and patch >= {})",
            version_bounds.lower_bound.major,
            version_bounds.lower_bound.minor,
            version_bounds.lower_bound.patch
        )
        .as_str(),
    );

    if !version_bounds.no_upper_bound {
        // construct upper bound based on number of components
        if version_bounds.num_parts == 1 {
            builder
                .push_str(format!(" AND (major < {})", version_bounds.upper_bound.major).as_str());
        } else if version_bounds.num_parts == 2
            || version_bounds.num_parts == 3 && version_bounds.parser_type == VersionParser::Tilde
            || version_bounds.num_parts == 3 && version_bounds.parser_type == VersionParser::Caret
        {
            builder.push_str(
                format!(
                    " AND (major = {} AND minor < {})",
                    version_bounds.upper_bound.major, version_bounds.upper_bound.minor
                )
                .as_str(),
            );
        } else {
            builder.push_str(
                format!(
                    " AND (major = {} AND minor = {} AND patch < {})",
                    version_bounds.upper_bound.major,
                    version_bounds.upper_bound.minor,
                    version_bounds.upper_bound.patch
                )
                .as_str(),
            );
        }
    }
    Ok(())
}

#[async_trait]
pub trait ProfileSqlLogic {
    /// Get profile versions
    #[instrument(skip_all)]
    async fn get_profile_versions(
        pool: &Pool<Postgres>,
        space: &str,
        name: &str,
        version: Option<String>,
    ) -> Result<Vec<String>, SqlError> {
        let mut version_query = Queries::GetProfileVersions.get_query().sql;

        if let Some(version) = version {
            add_version_bounds(&mut version_query, &version)?;
        }
        version_query.push_str(" ORDER BY created_at DESC LIMIT 20;");

        let cards: Vec<VersionResult> = sqlx::query_as(&version_query)
            .bind(space)
            .bind(name)
            .fetch_all(pool)
            .await?;

        let versions = cards
            .iter()
            .map(|c| c.to_version())
            .collect::<Result<Vec<Version>, SqlError>>()?;

        // sort semvers
        Ok(VersionValidator::sort_semver_versions(versions, true)?)
    }
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
        // we first need to determine correct version
        let base_args = drift_profile.get_base_args();

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
        pool: &Pool<Postgres>,
    ) -> Result<Option<TaskRequest>, SqlError> {
        let query = Queries::GetDriftTask.get_query();
        sqlx::query_as(&query.sql)
            .fetch_optional(pool)
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
        pool: &Pool<Postgres>,
        task_info: &DriftTaskInfo,
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
            .bind(&task_info.uid)
            .execute(pool)
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
