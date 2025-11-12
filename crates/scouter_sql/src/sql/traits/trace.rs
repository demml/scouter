use crate::sql::error::SqlError;
use crate::sql::query::Queries;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use itertools::multiunzip;
use scouter_types::sql::{TraceFilters, TraceListItem, TraceMetricBucket, TraceSpan};
use scouter_types::{TraceBaggageRecord, TraceRecord, TraceSpanRecord};
use sqlx::{postgres::PgQueryResult, types::Json, Pool, Postgres};

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
        traces: &[TraceRecord],
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
        spans: &[TraceSpanRecord],
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
            .bind(attributes)
            .bind(events)
            .bind(links)
            .bind(labels)
            .bind(input)
            .bind(output)
            .execute(pool)
            .await?;

        Ok(query_result)
    }

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
            )
        }));

        let query_result = sqlx::query(&query.sql)
            .bind(created_at)
            .bind(trace_id)
            .bind(scope)
            .bind(key)
            .bind(value)
            .execute(pool)
            .await?;

        Ok(query_result)
    }

    async fn get_trace_baggage_records(
        pool: &Pool<Postgres>,
        trace_id: &str,
    ) -> Result<Vec<TraceBaggageRecord>, SqlError> {
        let query = Queries::GetTraceBaggage.get_query();

        let baggage_items: Result<Vec<TraceBaggageRecord>, SqlError> = sqlx::query_as(&query.sql)
            .bind(trace_id)
            .fetch_all(pool)
            .await
            .map_err(SqlError::SqlxError);

        baggage_items
    }

    /// Attempts to retrieve paginated trace records from the database based on provided filters.
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `filters` - The filters to apply for retrieving traces
    /// # Returns
    /// * A vector of `TraceListItem` matching the filters
    async fn get_traces_paginated(
        pool: &Pool<Postgres>,
        filters: TraceFilters,
    ) -> Result<Vec<TraceListItem>, SqlError> {
        let default_start = Utc::now() - chrono::Duration::hours(24);
        let default_end = Utc::now();

        let query = Queries::GetPaginatedTraces.get_query();

        let trace_items: Result<Vec<TraceListItem>, SqlError> = sqlx::query_as(&query.sql)
            .bind(filters.space)
            .bind(filters.name)
            .bind(filters.version)
            .bind(filters.service_name)
            .bind(filters.has_errors)
            .bind(filters.status)
            .bind(filters.start_time.unwrap_or(default_start))
            .bind(filters.end_time.unwrap_or(default_end))
            .bind(filters.limit.unwrap_or(50))
            .bind(filters.cursor_created_at)
            .bind(filters.cursor_trace_id)
            .fetch_all(pool)
            .await
            .map_err(SqlError::SqlxError);

        trace_items
    }

    /// Attempts to retrieve trace spans for a given trace ID.
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `trace_id` - The trace ID to retrieve spans for
    /// # Returns
    /// * A vector of `TraceSpan` associated with the trace ID
    async fn get_trace_spans(
        pool: &Pool<Postgres>,
        trace_id: &str,
    ) -> Result<Vec<TraceSpan>, SqlError> {
        let query = Queries::GetTraceSpans.get_query();
        let trace_items: Result<Vec<TraceSpan>, SqlError> = sqlx::query_as(&query.sql)
            .bind(trace_id)
            .fetch_all(pool)
            .await
            .map_err(SqlError::SqlxError);

        trace_items
    }

    /// Attempts to retrieve trace spans for a given trace ID.
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `trace_id` - The trace ID to retrieve spans for
    /// # Returns
    /// * A vector of `TraceSpan` associated with the trace ID
    async fn get_trace_metrics(
        pool: &Pool<Postgres>,
        space: Option<&str>,
        name: Option<&str>,
        version: Option<&str>,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        bucket_interval_str: &str,
    ) -> Result<Vec<TraceMetricBucket>, SqlError> {
        let query = Queries::GetTraceMetrics.get_query();
        let trace_items: Result<Vec<TraceMetricBucket>, SqlError> = sqlx::query_as(&query.sql)
            .bind(space)
            .bind(name)
            .bind(version)
            .bind(start_time)
            .bind(end_time)
            .bind(bucket_interval_str)
            .fetch_all(pool)
            .await
            .map_err(SqlError::SqlxError);

        trace_items
    }
}
