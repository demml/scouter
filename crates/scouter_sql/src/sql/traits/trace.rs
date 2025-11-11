use crate::sql::error::SqlError;
use crate::sql::query::Queries;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use itertools::multiunzip;
use scouter_types::{TraceBaggageRecord, TraceRecord, TraceSpanRecord};
use sqlx::{postgres::PgQueryResult, types::Json, Pool, Postgres};
use std::result::Result::Ok;

#[async_trait]
pub trait TraceSqlLogic {
    /// Attempts to upsert multiple trace records into the database in a batch.
    ///
    /// # Arguments
    ///
    /// * `pool` - The database connection pool
    /// * `traces` - The trace records to insert
    async fn upsert_trace_batch(
        pool: &Pool<Postgres>,
        traces: &Vec<TraceRecord>,
    ) -> Result<PgQueryResult, SqlError> {
        let query = Queries::UpsertTrace.get_query();
        let capacity = traces.len();

        // Pre-allocate vectors for each field for batch efficiency
        let mut created_at = Vec::with_capacity(capacity);
        let mut trace_id = Vec::with_capacity(capacity);
        let mut space = Vec::with_capacity(capacity);
        let mut name = Vec::with_capacity(capacity);
        let mut version = Vec::with_capacity(capacity);
        let mut scope = Vec::with_capacity(capacity);
        let mut trace_state = Vec::with_capacity(capacity);
        let mut start_time = Vec::with_capacity(capacity);
        let mut end_time = Vec::with_capacity(capacity);
        let mut duration_ms = Vec::with_capacity(capacity);
        let mut status = Vec::with_capacity(capacity);
        let mut root_span_id = Vec::with_capacity(capacity);
        let mut attributes = Vec::with_capacity(capacity);

        // Single-pass extraction for performance
        for r in traces {
            created_at.push(r.created_at);
            trace_id.push(r.trace_id.as_str());
            space.push(r.space.as_str());
            name.push(r.name.as_str());
            version.push(r.version.as_str());
            scope.push(r.scope.as_str());
            trace_state.push(r.trace_state.as_str());
            start_time.push(r.start_time);
            end_time.push(r.end_time);
            duration_ms.push(r.duration_ms);
            status.push(r.status.as_str());
            root_span_id.push(r.root_span_id.as_str());
            attributes.push(Json(r.attributes.clone()));
        }

        let query_result = sqlx::query(&query.sql)
            .bind(created_at)
            .bind(trace_id)
            .bind(space)
            .bind(name)
            .bind(version)
            .bind(scope)
            .bind(trace_state)
            .bind(start_time)
            .bind(end_time)
            .bind(duration_ms)
            .bind(status)
            .bind(root_span_id)
            .bind(attributes)
            .execute(pool)
            .await?;

        Ok(query_result)
    }

    /// Attempts to insert multiple trace span records into the database in a batch.
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `spans` - The trace span records to insert
    async fn insert_span_batch(
        pool: &Pool<Postgres>,
        spans: &Vec<TraceSpanRecord>,
    ) -> Result<PgQueryResult, SqlError> {
        let query = Queries::InsertTraceSpan.get_query();
        let capacity = spans.len();

        // we are pre-allocating here instead of using multiunzip because multiunzip has
        // a max limit of 12 tuples (we have 18 fields)
        let mut created_at = Vec::with_capacity(capacity);
        let mut span_id = Vec::with_capacity(capacity);
        let mut trace_id = Vec::with_capacity(capacity);
        let mut parent_span_id = Vec::with_capacity(capacity);
        let mut space = Vec::with_capacity(capacity);
        let mut name = Vec::with_capacity(capacity);
        let mut version = Vec::with_capacity(capacity);
        let mut scope = Vec::with_capacity(capacity);
        let mut span_name = Vec::with_capacity(capacity);
        let mut span_kind = Vec::with_capacity(capacity);
        let mut start_time = Vec::with_capacity(capacity);
        let mut end_time = Vec::with_capacity(capacity);
        let mut duration_ms = Vec::with_capacity(capacity);
        let mut status_code = Vec::with_capacity(capacity);
        let mut status_message = Vec::with_capacity(capacity);
        let mut attributes = Vec::with_capacity(capacity);
        let mut events = Vec::with_capacity(capacity);
        let mut links = Vec::with_capacity(capacity);
        let mut labels = Vec::with_capacity(capacity);
        let mut input = Vec::with_capacity(capacity);
        let mut output = Vec::with_capacity(capacity);

        // Single iteration for maximum efficiency
        for span in spans {
            created_at.push(span.created_at);
            span_id.push(span.span_id.as_str());
            trace_id.push(span.trace_id.as_str());
            parent_span_id.push(span.parent_span_id.as_deref());
            space.push(span.space.as_str());
            name.push(span.name.as_str());
            version.push(span.version.as_str());
            scope.push(span.scope.as_str());
            span_name.push(span.span_name.as_str());
            span_kind.push(span.span_kind.as_str());
            start_time.push(span.start_time);
            end_time.push(span.end_time);
            duration_ms.push(span.duration_ms);
            status_code.push(span.status_code.as_str());
            status_message.push(span.status_message.as_str());
            attributes.push(Json(span.attributes.clone()));
            events.push(Json(span.events.clone()));
            links.push(Json(span.links.clone()));
            labels.push(span.label.as_deref());
            input.push(Json(span.input.clone()));
            output.push(Json(span.output.clone()));
        }

        let query_result = sqlx::query(&query.sql)
            .bind(created_at)
            .bind(span_id)
            .bind(trace_id)
            .bind(parent_span_id)
            .bind(space)
            .bind(name)
            .bind(version)
            .bind(scope)
            .bind(span_name)
            .bind(span_kind)
            .bind(start_time)
            .bind(end_time)
            .bind(duration_ms)
            .bind(status_code)
            .bind(status_message)
            .bind(Json(attributes))
            .bind(Json(events))
            .bind(Json(links))
            .bind(labels)
            .bind(Json(input))
            .bind(Json(output))
            .execute(pool)
            .await?;

        Ok(query_result)
    }

    /// Attempts to insert multiple trace baggage records into the database in a batch.
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `baggage` - The trace baggage records to insert
    async fn insert_baggage_batch(
        pool: &Pool<Postgres>,
        baggage: &[TraceBaggageRecord],
    ) -> Result<PgQueryResult, SqlError> {
        let query = Queries::InsertTraceBaggage.get_query();

        let (created_at, trace_id, scope, key, value, space, name, version): (
            Vec<DateTime<Utc>>,
            Vec<&str>,
            Vec<&str>,
            Vec<&str>,
            Vec<&str>,
            Vec<&str>,
            Vec<&str>,
            Vec<&str>,
        ) = multiunzip(baggage.iter().map(|b| {
            (
                b.created_at,
                b.trace_id.as_str(),
                b.scope.as_str(),
                b.key.as_str(),
                b.value.as_str(),
                b.space.as_str(),
                b.name.as_str(),
                b.version.as_str(),
            )
        }));

        let query_result = sqlx::query(&query.sql)
            .bind(created_at)
            .bind(trace_id)
            .bind(scope)
            .bind(key)
            .bind(value)
            .bind(space)
            .bind(name)
            .bind(version)
            .execute(pool)
            .await?;

        Ok(query_result)
    }
}
