use crate::sql::query::Queries;
use crate::sql::schema::{AlertWrapper, UpdateAlertResult};

use scouter_types::contracts::{DriftAlertRequest, UpdateAlertStatus};

use crate::sql::error::SqlError;
use scouter_types::alert::Alert;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use itertools::multiunzip;
use scouter_types::{TraceRecord, TraceSpanRecord};
use sqlx::{postgres::PgQueryResult, Pool, Postgres};
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

        let (
            trace_id,
            space,
            name,
            version,
            scope,
            trace_state,
            start_time,
            end_time,
            duration_ms,
            status,
            root_span_id,
            attributes,
        ): (
            Vec<&str>,
            Vec<&str>,
            Vec<&str>,
            Vec<&str>,
            Vec<&str>,
            Vec<&str>,
            Vec<DateTime<Utc>>,
            Vec<DateTime<Utc>>,
            Vec<i64>,
            Vec<&str>,
            Vec<&str>,
            Vec<Option<serde_json::Value>>,
        ) = multiunzip(traces.iter().map(|r| {
            (
                r.trace_id.as_str(),
                r.space.as_str(),
                r.name.as_str(),
                r.version.as_str(),
                r.scope.as_str(),
                r.trace_state.as_str(),
                r.start_time,
                r.end_time,
                r.duration_ms,
                r.status.as_str(),
                r.root_span_id.as_str(),
                r.attributes.clone(),
            )
        }));

        let query_result = sqlx::query(&query.sql)
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
        // a max limit of 12 tuples (we have 17 fields)
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

        // Single iteration for maximum efficiency
        for span in spans {
            span_id.push(span.span_id.as_str());
            trace_id.push(span.trace_id.as_str());
            parent_span_id.push(span.parent_span_id.as_ref().map(|s| s.as_str()));
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
            attributes.push(span.attributes.clone());
            events.push(span.events.clone());
            links.push(span.links.clone());
        }

        let query_result = sqlx::query(&query.sql)
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
            .bind(attributes)
            .bind(events)
            .bind(links)
            .execute(pool)
            .await?;

        Ok(query_result)
    }
}
