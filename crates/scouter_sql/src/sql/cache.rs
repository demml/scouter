use crate::sql::error::SqlError;
use crate::sql::traits::EntitySqlLogic;
use crate::PostgresClient;
use mini_moka::sync::Cache;
use scouter_types::DriftType;
use sqlx::{Pool, Postgres};
use std::sync::OnceLock;

#[derive(Clone, Debug)]
pub struct EntityCache {
    pool: Pool<Postgres>,
    cache: Cache<String, i32>,
}

impl EntityCache {
    pub fn new(pool: Pool<Postgres>, max_capacity: u64) -> Self {
        let cache = Cache::new(max_capacity);
        Self { pool, cache }
    }

    /// Get entity ID from UID with caching
    ///  # Arguments
    /// * `uid` - The UID of the entity
    /// # Returns
    /// * `Result<i32, SqlError>` - Result of the query returning the entity
    pub async fn get_entity_id_from_uid(&self, uid: &String) -> Result<i32, SqlError> {
        match self.cache.get(uid) {
            Some(cached_id) => Ok(cached_id),
            None => {
                let entity_id = PostgresClient::get_entity_id_from_uid(&self.pool, uid).await?;
                self.cache.insert(uid.to_string(), entity_id);
                Ok(entity_id)
            }
        }
    }

    pub async fn get_optional_entity_id_from_uid(
        &self,
        uid: &String,
    ) -> Result<Option<i32>, SqlError> {
        match self.cache.get(uid) {
            Some(cached_id) => Ok(Some(cached_id)),
            None => {
                let entity_id =
                    PostgresClient::get_optional_entity_id_from_uid(&self.pool, uid).await?;
                if let Some(id) = entity_id {
                    self.cache.insert(uid.to_string(), id);
                }
                Ok(entity_id)
            }
        }
    }

    /// helper for getting entity id from space, name, version, drift_type
    pub async fn get_entity_id_from_space_name_version_drift_type(
        &self,
        space: &String,
        name: &String,
        version: &String,
        drift_type: &DriftType,
    ) -> Result<i32, SqlError> {
        let id = PostgresClient::get_entity_id_from_space_name_version_drift_type(
            &self.pool,
            space,
            name,
            version,
            drift_type.to_string(),
        )
        .await?;

        Ok(id)
    }
}

// Global instance of the application state manager
static INSTANCE: OnceLock<EntityCache> = OnceLock::new();

pub fn init_entity_cache(pool: Pool<Postgres>, max_capacity: u64) {
    let cache = EntityCache::new(pool, max_capacity);
    INSTANCE
        .set(cache)
        .expect("EntityCache has already been initialized");
}

pub fn entity_cache() -> &'static EntityCache {
    INSTANCE.get().expect("EntityCache is not initialized")
}
