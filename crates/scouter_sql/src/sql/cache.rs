/// This is a global Cache oncelock for accessing the entity_id cache.
/// We found that we we're clonging an passing the same cache across multiple functions.
/// It's easier just to use it as a global oncelock
pub use error::StateError;
use std::future::Future;
use std::sync::{Arc, OnceLock};
use tokio::runtime::{Handle, Runtime};
use tracing::debug;

use mini_moka::sync::Cache;
use scouter_sql::sql::traits::EntitySqlLogic;
use scouter_sql::PostgresClient;
use sqlx::{Pool, Postgres};

pub struct EntityCache {
    pool: Pool<Postgres>,
    cache: Cache<String, i32>,
}

impl EntityCache {
    pub fn new(pool: Pool<Postgres>, max_capacity: usize) -> Self {
        let cache = Cache::new(max_capacity);
        Self { pool, cache }
    }

    /// Get entity ID from UID with caching
    ///  # Arguments
    /// * `uid` - The UID of the entity
    /// # Returns
    /// * `Result<i32, SqlError>` - Result of the query returning the entity
    pub async fn get_entity_id_from_uid(&self, uid: &str) -> Result<i32, ServerError> {
        match self.cache.get(uid) {
            Some(cached_id) => Ok(cached_id),
            None => {
                let entity_id = PostgresClient::get_entity_id_from_uid(&self.db_pool, uid).await?;
                self.cache.insert(uid.to_string(), entity_id);
                Ok(entity_id)
            }
        }
    }

    pub async fn get_optional_entity_id_from_uid(
        &self,
        uid: &str,
    ) -> Result<Option<i32>, ServerError> {
        match self.cache.get(uid) {
            Some(cached_id) => Ok(Some(cached_id)),
            None => {
                let entity_id =
                    PostgresClient::get_optional_entity_id_from_uid(&self.db_pool, uid).await?;
                if let Some(id) = entity_id {
                    self.cache.insert(uid.to_string(), id);
                }
                Ok(entity_id)
            }
        }
    }
}

// Global instance of the application state manager
static INSTANCE: OnceLock<EntityCache> = OnceLock::new();

pub fn init_entity_cache(pool: Pool<Postgres>, max_capacity: usize) {
    let cache = EntityCache::new(pool, max_capacity);
    INSTANCE
        .set(cache)
        .expect("EntityCache has already been initialized");
}

pub fn entity_cache() -> &'static EntityCache {
    INSTANCE.get().expect("EntityCache is not initialized")
}
