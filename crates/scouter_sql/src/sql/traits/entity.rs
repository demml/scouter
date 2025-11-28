use crate::sql::query::Queries;
use crate::sql::schema::TaskRequest;

use crate::sql::error::SqlError;
use crate::sql::schema::VersionResult;
use async_trait::async_trait;
use chrono::Utc;
use cron::Schedule;
use scouter_semver::VersionArgs;
use scouter_semver::VersionType;
use scouter_semver::{VersionParser, VersionValidator};
use scouter_types::{
    DriftProfile, GetProfileRequest, ListProfilesRequest, ListedProfile, ProfileArgs,
    ProfileStatusRequest,
};
use semver::Version;
use serde_json::Value;
use sqlx::{postgres::PgQueryResult, Pool, Postgres, Row};
use std::result::Result::Ok;
use std::str::FromStr;
use tracing::{error, instrument};

#[async_trait]
pub trait EntitySqlLogic {
    /// Get entity ID from UID
    /// # Arguments
    /// * `uid` - The UID of the entity
    /// # Returns
    /// * `Result<i32, SqlError>` - Result of the query returning the entity ID
    async fn get_entity_id_from_uid(pool: &Pool<Postgres>, uid: &str) -> Result<i32, SqlError> {
        let query = Queries::GetEntityIdFromUid.get_query();

        let result = sqlx::query(&query.sql)
            .bind(uid)
            .fetch_one(pool)
            .await
            .map_err(SqlError::SqlxError)?;

        let id: i32 = result.get("id");

        Ok(id)
    }

    async fn get_entity_id_from_space_name_version_drift_type(
        pool: &Pool<Postgres>,
        space: &str,
        name: &str,
        version: &str,
        drift_type: &str,
    ) -> Result<i32, SqlError> {
        let query = Queries::GetEntityIdFromSpaceNameVersionDriftType.get_query();

        let result = sqlx::query(&query.sql)
            .bind(space)
            .bind(name)
            .bind(version)
            .bind(drift_type)
            .fetch_one(pool)
            .await
            .map_err(SqlError::SqlxError)?;

        let id: i32 = result.get("id");

        Ok(id)
    }
}
