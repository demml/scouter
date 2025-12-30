use crate::sql::error::SqlError;
use crate::sql::query::Queries;
use async_trait::async_trait;
use sqlx::{Pool, Postgres, Row};
use std::result::Result::Ok;

#[async_trait]
pub trait EntitySqlLogic {
    /// Get entity ID from UID
    /// # Arguments
    /// * `uid` - The UID of the entity
    /// # Returns
    /// * `Result<i32, SqlError>` - Result of the query returning the entity ID
    async fn get_entity_id_from_uid(pool: &Pool<Postgres>, uid: &str) -> Result<i32, SqlError> {
        let query = Queries::GetEntityIdFromUid.get_query();

        let result = sqlx::query(query)
            .bind(uid)
            .fetch_one(pool)
            .await
            .map_err(SqlError::SqlxError)?;

        let id: i32 = result.get("id");

        Ok(id)
    }

    async fn get_optional_entity_id_from_uid(
        pool: &Pool<Postgres>,
        uid: &str,
    ) -> Result<Option<i32>, SqlError> {
        let query = Queries::GetEntityIdFromUid.get_query();

        sqlx::query(query)
            .bind(uid)
            .fetch_optional(pool)
            .await
            .map_err(SqlError::SqlxError)
            .map(|row| row.map(|r| r.get("id")))
    }

    async fn get_entity_id_from_space_name_version_drift_type(
        pool: &Pool<Postgres>,
        space: &str,
        name: &str,
        version: &str,
        drift_type: &str,
    ) -> Result<i32, SqlError> {
        let query = Queries::GetEntityIdFromSpaceNameVersionDriftType.get_query();

        let result = sqlx::query(query)
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

    /// Helper function to create a new entity
    /// # Arguments
    /// * `space` - The space of the entity
    /// * `name` - The name of the entity
    /// * `version` - The version of the entity
    /// * `drift_type` - The drift type of the entity
    /// # Returns
    /// * `Result<String, SqlError>` - Result of the insert returning the new
    async fn create_entity(
        pool: &Pool<Postgres>,
        space: &str,
        name: &str,
        version: &str,
        drift_type: &str,
    ) -> Result<(String, i32), SqlError> {
        let query = "INSERT INTO scouter.drift_entities (space, name, version, drift_type) VALUES ($1, $2, $3, $4) ON CONFLICT (space, name, version, drift_type) DO NOTHING RETURNING id, uid;";

        let result = sqlx::query(query)
            .bind(space)
            .bind(name)
            .bind(version)
            .bind(drift_type)
            .fetch_one(pool)
            .await
            .map_err(SqlError::SqlxError)?;

        let uid: String = result.get("uid");
        let id: i32 = result.get("id");

        Ok((uid, id))
    }

    async fn get_uid_from_args(
        pool: &Pool<Postgres>,
        space: &str,
        name: &str,
        version: &str,
        drift_type: &str,
    ) -> Result<String, SqlError> {
        let query = "SELECT uid FROM entities WHERE space = $1 AND name = $2 AND version = $3 AND drift_type = $4;";

        let result = sqlx::query(query)
            .bind(space)
            .bind(name)
            .bind(version)
            .bind(drift_type)
            .fetch_one(pool)
            .await
            .map_err(SqlError::SqlxError)?;

        let uid: String = result.get("uid");

        Ok(uid)
    }
}
