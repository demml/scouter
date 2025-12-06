use crate::sql::error::SqlError;
use crate::sql::traits::EntitySqlLogic;
use crate::PostgresClient;
use mini_moka::sync::Cache;
use scouter_types::DriftType;
use sqlx::{Pool, Postgres};
use std::sync::OnceLock;

#[derive(Clone, Debug)]
pub struct EntityCache {
    cache: Cache<String, i32>,
}

impl EntityCache {
    pub fn new(max_capacity: u64) -> Self {
        let cache = Cache::new(max_capacity);
        Self { cache }
    }

    /// Get entity ID from UID with caching
    /// # Arguments
    /// * `pool` - Database pool to use for queries
    /// * `uid` - The UID of the entity
    /// # Returns
    /// * `Result<i32, SqlError>` - Result of the query returning the entity
    pub async fn get_entity_id_from_uid(
        &self,
        pool: &Pool<Postgres>,
        uid: &String,
    ) -> Result<i32, SqlError> {
        match self.cache.get(uid) {
            Some(cached_id) => Ok(cached_id),
            None => {
                let entity_id = PostgresClient::get_entity_id_from_uid(pool, uid).await?;
                self.cache.insert(uid.to_string(), entity_id);
                Ok(entity_id)
            }
        }
    }

    pub async fn get_optional_entity_id_from_uid(
        &self,
        pool: &Pool<Postgres>,
        uid: &String,
    ) -> Result<Option<i32>, SqlError> {
        match self.cache.get(uid) {
            Some(cached_id) => Ok(Some(cached_id)),
            None => {
                let entity_id = PostgresClient::get_optional_entity_id_from_uid(pool, uid).await?;
                if let Some(id) = entity_id {
                    self.cache.insert(uid.to_string(), id);
                }
                Ok(entity_id)
            }
        }
    }

    pub async fn get_entity_id_from_space_name_version_drift_type(
        &self,
        pool: &Pool<Postgres>,
        space: &str,
        name: &str,
        version: &str,
        drift_type: &DriftType,
    ) -> Result<i32, SqlError> {
        let id = PostgresClient::get_entity_id_from_space_name_version_drift_type(
            pool,
            space,
            name,
            version,
            drift_type.to_string(),
        )
        .await?;

        Ok(id)
    }
}

static INSTANCE: OnceLock<EntityCache> = OnceLock::new();

pub fn init_entity_cache(max_capacity: u64) {
    INSTANCE.get_or_init(|| {
        tracing::info!("Initializing EntityCache");
        EntityCache::new(max_capacity)
    });
}

pub fn entity_cache() -> &'static EntityCache {
    INSTANCE
        .get()
        .expect("EntityCache is not initialized - call init_entity_cache first")
}
