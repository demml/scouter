use crate::sql::error::SqlError;
use crate::sql::query::Queries;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use itertools::multiunzip;
use scouter_types::TagRecord;
use sqlx::{postgres::PgQueryResult, Pool, Postgres};
use std::result::Result::Ok;

#[async_trait]
pub trait TagSqlLogic {
    /// Attempts to insert multiple trace baggage records into the database in a batch.
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `baggage` - The trace baggage records to insert
    async fn insert_tag_batch(
        pool: &Pool<Postgres>,
        tags: &[TagRecord],
    ) -> Result<PgQueryResult, SqlError> {
        let query = Queries::InsertTag.get_query();

        let (created_at, entity_type, entity_id, key, value): (
            Vec<DateTime<Utc>>,
            Vec<&str>,
            Vec<&str>,
            Vec<&str>,
            Vec<&str>,
        ) = multiunzip(tags.iter().map(|b| {
            (
                b.created_at,
                b.entity_type.as_str(),
                b.entity_id.as_str(),
                b.key.as_str(),
                b.value.as_str(),
            )
        }));

        let query_result = sqlx::query(&query.sql)
            .bind(created_at)
            .bind(entity_type)
            .bind(entity_id)
            .bind(key)
            .bind(value)
            .execute(pool)
            .await?;

        Ok(query_result)
    }

    async fn get_tags(
        pool: &Pool<Postgres>,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Vec<TagRecord>, SqlError> {
        let query = Queries::GetTags.get_query();

        let rows = sqlx::query_as::<_, TagRecord>(&query.sql)
            .bind(entity_type)
            .bind(entity_id)
            .fetch_all(pool)
            .await?;

        Ok(rows)
    }
}
