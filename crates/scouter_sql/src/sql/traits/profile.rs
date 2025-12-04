use crate::sql::query::Queries;
use crate::sql::schema::TaskRequest;

use crate::sql::error::SqlError;
use crate::sql::schema::VersionResult;
use async_trait::async_trait;
use chrono::Utc;
use cron::Schedule;
use potato_head::create_uuid7;
use scouter_semver::VersionArgs;
use scouter_semver::{VersionParser, VersionValidator};
use scouter_types::VersionRequest;
use scouter_types::{
    DriftProfile, ListProfilesRequest, ListedProfile, ProfileArgs, ProfileStatusRequest,
};
use semver::Version;
use serde_json::Value;
use sqlx::{postgres::PgQueryResult, Pool, Postgres, Row};
use std::result::Result::Ok;
use std::str::FromStr;
use tracing::{error, instrument};

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
    /// Determines the next version based on existing versions in the database
    /// # Arguments
    /// * `args` - The profile arguments containing space, name, and version
    /// * `version_type` - The type of version bump (major, minor, patch)
    /// * `pre_tag` - Optional pre-release tag
    /// * `build_tag` - Optional build metadata
    /// # Returns
    /// * `Result<Version, SqlError>` - Result of the query returning
    #[instrument(skip_all)]
    async fn get_next_profile_version(
        pool: &Pool<Postgres>,
        args: &ProfileArgs,
        version_request: VersionRequest,
    ) -> Result<Version, SqlError> {
        let mut version_query = Queries::GetProfileVersions.get_query().sql;

        if let Some(version) = &version_request.version {
            add_version_bounds(&mut version_query, version)?;
        }
        version_query.push_str(" ORDER BY created_at DESC LIMIT 20;");

        let cards: Vec<VersionResult> = sqlx::query_as(&version_query)
            .bind(&args.space)
            .bind(&args.name)
            .fetch_all(pool)
            .await?;

        let versions = cards
            .iter()
            .map(|c| c.to_version())
            .collect::<Result<Vec<Version>, SqlError>>()?;

        // sort semvers
        let versions = VersionValidator::sort_semver_versions(versions, true)?;

        if versions.is_empty() {
            return match &version_request.version {
                Some(version_str) => Ok(VersionValidator::clean_version(version_str)?),
                None => Ok(Version::new(0, 1, 0)),
            };
        }

        let base_version = versions.first().unwrap().to_string();

        let args = VersionArgs {
            version: base_version,
            version_type: version_request.version_type,
            pre: version_request.pre_tag,
            build: version_request.build_tag,
        };

        Ok(VersionValidator::bump_version(&args)?)
    }
    /// Insert a drift profile into the database
    ///
    /// # Arguments
    ///
    /// * `drift_profile` - The drift profile to insert
    ///
    /// # Returns
    ///
    /// * `Result<String, SqlError>` - Result of the query returning the entity_uid
    #[instrument(skip_all)]
    async fn insert_drift_profile(
        pool: &Pool<Postgres>,
        drift_profile: &DriftProfile,
        base_args: &ProfileArgs,
        version: &Version,
        active: &bool,
        deactivate_others: &bool,
    ) -> Result<String, SqlError> {
        let query = Queries::InsertDriftProfile.get_query();
        let current_time = Utc::now();
        let schedule = Schedule::from_str(&base_args.schedule)?;
        let next_run = match schedule.upcoming(Utc).take(1).next() {
            Some(next_run) => next_run,
            None => {
                return Err(SqlError::GetNextRunError);
            }
        };

        // Need to convert version to postgres type
        let major = version.major as i32;
        let minor = version.minor as i32;
        let patch = version.patch as i32;
        let pre: Option<String> = version.pre.to_string().parse().ok();
        let build: Option<String> = version.build.to_string().parse().ok();

        let result = sqlx::query(&query.sql)
            // 1. Entity UID (for entity_insert) -> $1
            .bind(create_uuid7())
            // 2-3. Entity Identity (for entity_insert & deactivation logic) -> $2, $3
            .bind(&base_args.space)
            .bind(&base_args.name)
            // 4-8. Version Components (for drift_profile insert) -> $4 to $8
            .bind(major)
            .bind(minor)
            .bind(patch)
            .bind(pre)
            .bind(build)
            // 9-11. String/JSON values -> $9 to $11
            .bind(version.to_string()) // Full Version String
            .bind(&base_args.scouter_version)
            .bind(drift_profile.to_value()) // Profile JSON
            // 12. Drift Type (for entity_insert & deactivation logic) -> $12
            .bind(base_args.drift_type.to_string())
            // 13. Active Flag (for deactivation logic & drift_profile insert) -> $13
            .bind(active)
            // 14. Schedule -> $14
            .bind(&base_args.schedule)
            // 15. Next Run -> $15
            .bind(next_run)
            // 16. Current Time (for previous_run/timestamps) -> $16
            .bind(current_time)
            // 17. Deactivate Others Flag (for deactivation logic) -> $17
            .bind(deactivate_others)
            .fetch_one(pool)
            .await
            .map_err(SqlError::SqlxError)?;

        let entity_uid: String = result.get("entity_uid");
        Ok(entity_uid)
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
        entity_id: &i32,
    ) -> Result<PgQueryResult, SqlError> {
        let query = Queries::UpdateDriftProfile.get_query();

        sqlx::query(&query.sql)
            .bind(drift_profile.to_value())
            .bind(drift_profile.drift_type().to_string())
            .bind(entity_id)
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
        entity_id: &i32,
    ) -> Result<Option<Value>, SqlError> {
        let query = Queries::GetDriftProfile.get_query();

        let result = sqlx::query(&query.sql)
            .bind(entity_id)
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

    async fn list_drift_profiles(
        pool: &Pool<Postgres>,
        args: &ListProfilesRequest,
    ) -> Result<Vec<ListedProfile>, SqlError> {
        let profile_query = Queries::ListDriftProfiles.get_query().sql;

        let records: Vec<(bool, Value)> = sqlx::query_as(&profile_query)
            .bind(&args.space)
            .bind(&args.name)
            .bind(&args.version)
            .fetch_all(pool)
            .await
            .map_err(SqlError::SqlxError)?;

        let listed_profiles: Vec<ListedProfile> = records
            .into_iter()
            .map(|(active, value)| -> Result<ListedProfile, SqlError> {
                Ok(ListedProfile {
                    profile: DriftProfile::from_value(value)?,
                    active,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(listed_profiles)
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
        entity_id: &i32,
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
            .bind(entity_id)
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
