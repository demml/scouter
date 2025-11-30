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
}
