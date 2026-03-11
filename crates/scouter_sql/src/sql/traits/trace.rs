use crate::sql::error::SqlError;
use crate::sql::query::Queries;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use itertools::multiunzip;
use scouter_types::sql::TraceSpan;
use scouter_types::{TraceBaggageRecord, TraceId};
use sqlx::{postgres::PgQueryResult, types::Json, Pool, Postgres};
use std::collections::HashMap;
use tracing::error;
#[async_trait]
pub trait TraceSqlLogic {
    /// Attempts to insert multiple trace baggage records into the database in a batch.
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `baggage` - The trace baggage records to insert
    async fn insert_trace_baggage_batch(
        pool: &Pool<Postgres>,
        baggage: &[TraceBaggageRecord],
    ) -> Result<PgQueryResult, SqlError> {
        let query = Queries::InsertTraceBaggage.get_query();

        let (created_at, trace_id, scope, key, value): (
            Vec<DateTime<Utc>>,
            Vec<&[u8]>,
            Vec<&str>,
            Vec<&str>,
            Vec<&str>,
        ) = multiunzip(baggage.iter().map(|b| {
            (
                b.created_at,
                b.trace_id.as_bytes() as &[u8],
                b.scope.as_str(),
                b.key.as_str(),
                b.value.as_str(),
            )
        }));

        let query_result = sqlx::query(query)
            .bind(created_at)
            .bind(trace_id)
            .bind(scope)
            .bind(key)
            .bind(value)
            .execute(pool)
            .await
            .inspect_err(|e| error!("Error inserting trace baggage: {:?}", e))?;

        Ok(query_result)
    }

    /// Attempts to retrieve trace baggage records for a given trace ID.
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `trace_id` - The trace ID to retrieve baggage for. This is always the hex encoded id
    /// # Returns
    /// * A vector of `TraceBaggageRecord` associated with the trace ID
    async fn get_trace_baggage_records(
        pool: &Pool<Postgres>,
        trace_id: &str,
    ) -> Result<Vec<TraceBaggageRecord>, SqlError> {
        let bytes = TraceId::hex_to_bytes(trace_id)?;

        let query = Queries::GetTraceBaggage.get_query();

        let baggage_items: Result<Vec<TraceBaggageRecord>, SqlError> = sqlx::query_as(query)
            .bind(bytes.as_slice())
            .fetch_all(pool)
            .await
            .map_err(SqlError::SqlxError);

        baggage_items
    }

    /// Attempts to retrieve trace spans based on tag filters.
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `entity_type` - The entity type to filter spans
    /// * `tag_filters` - The tag filters to apply
    /// * `match_all` - Whether to match all tags or any
    /// * `service_name` - Optional service name to filter spans
    /// # Returns
    /// * A vector of `TraceSpan` matching the tag filters
    async fn get_spans_from_tags(
        pool: &Pool<Postgres>,
        entity_type: &str,
        tag_filters: Vec<HashMap<String, String>>,
        match_all: bool,
        service_name: Option<&str>,
    ) -> Result<Vec<TraceSpan>, SqlError> {
        let query = Queries::GetSpansByTags.get_query();

        sqlx::query_as(query)
            .bind(entity_type)
            .bind(Json(tag_filters))
            .bind(match_all)
            .bind(service_name)
            .fetch_all(pool)
            .await
            .map_err(SqlError::SqlxError)
    }

    /// Resolve `entity_uid` (UUID string) to raw 16-byte trace IDs via `scouter.trace_entities`.
    /// Returns empty `Vec` when `entity_uid` is invalid or no rows match.
    async fn get_trace_ids_for_entity(
        pool: &Pool<Postgres>,
        entity_uid: &str,
    ) -> Result<Vec<Vec<u8>>, SqlError> {
        let uuid: uuid::Uuid = entity_uid.parse().map_err(SqlError::UuidError)?;
        let uid_bytes = uuid.as_bytes().to_vec();
        sqlx::query_scalar::<_, Vec<u8>>(
            "SELECT trace_id FROM scouter.trace_entities WHERE entity_uid = $1",
        )
        .bind(uid_bytes)
        .fetch_all(pool)
        .await
        .map_err(SqlError::SqlxError)
    }
}
