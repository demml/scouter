use crate::sql::error::SqlError;
use crate::sql::query::Queries;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use itertools::multiunzip;
use scouter_types::sql::{TraceFilters, TraceListItem, TraceMetricBucket, TraceSpan};
use scouter_types::{TraceBaggageRecord, TraceCursor, TracePaginationResponse, TraceSpanRecord};
use sqlx::{postgres::PgQueryResult, types::Json, Pool, Postgres};

#[async_trait]
pub trait TraceSqlLogic {
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
        let mut service_name = Vec::with_capacity(capacity);
        let mut resource_attributes = Vec::with_capacity(capacity);

        // Single iteration for maximum efficiency
        for span in spans {
            created_at.push(span.created_at);
            span_id.push(span.span_id.as_str());
            trace_id.push(span.trace_id.as_str());
            parent_span_id.push(span.parent_span_id.as_deref());
            scope.push(span.scope.as_str());
            span_name.push(span.span_name.as_str());
            span_kind.push(span.span_kind.as_str());
            start_time.push(span.start_time);
            end_time.push(span.end_time);
            duration_ms.push(span.duration_ms);
            status_code.push(span.status_code);
            status_message.push(span.status_message.as_str());
            attributes.push(Json(span.attributes.clone()));
            events.push(Json(span.events.clone()));
            links.push(Json(span.links.clone()));
            labels.push(span.label.as_deref());
            input.push(Json(span.input.clone()));
            output.push(Json(span.output.clone()));
            service_name.push(span.service_name.as_str());
            resource_attributes.push(Json(span.resource_attributes.clone()));
        }

        let query_result = sqlx::query(query)
            .bind(created_at)
            .bind(span_id)
            .bind(trace_id)
            .bind(parent_span_id)
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
            .bind(service_name)
            .bind(resource_attributes)
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

        let query_result = sqlx::query(query)
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

        let baggage_items: Result<Vec<TraceBaggageRecord>, SqlError> = sqlx::query_as(query)
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
    ) -> Result<TracePaginationResponse, SqlError> {
        let default_start = Utc::now() - chrono::Duration::hours(24);
        let default_end = Utc::now();
        let limit = filters.limit.unwrap_or(50);
        let direction = filters.direction.as_deref().unwrap_or("next");

        let query = Queries::GetPaginatedTraces.get_query();

        let mut items: Vec<TraceListItem> = sqlx::query_as(query)
            .bind(filters.service_name)
            .bind(filters.has_errors)
            .bind(filters.status_code)
            .bind(filters.start_time.unwrap_or(default_start))
            .bind(filters.end_time.unwrap_or(default_end))
            .bind(limit)
            .bind(filters.cursor_created_at)
            .bind(filters.cursor_trace_id)
            .bind(direction)
            .fetch_all(pool)
            .await
            .map_err(SqlError::SqlxError)?;

        let has_more = items.len() > limit as usize;

        // Remove the extra item
        if has_more {
            items.pop();
        }

        // Determine next/previous based on direction
        let (has_next, next_cursor, has_previous, previous_cursor) = match direction {
            "next" => {
                // Forward pagination
                let next_cursor = if has_more {
                    items.last().map(|last| TraceCursor {
                        created_at: last.created_at,
                        trace_id: last.trace_id.clone(),
                    })
                } else {
                    None
                };

                let previous_cursor = items.first().map(|first| TraceCursor {
                    created_at: first.created_at,
                    trace_id: first.trace_id.clone(),
                });

                (
                    has_more,
                    next_cursor,
                    filters.cursor_created_at.is_some(),
                    previous_cursor,
                )
            }
            "previous" => {
                // Backward pagination
                let previous_cursor = if has_more {
                    items.first().map(|first| TraceCursor {
                        created_at: first.created_at,
                        trace_id: first.trace_id.clone(),
                    })
                } else {
                    None
                };

                let next_cursor = items.last().map(|last| TraceCursor {
                    created_at: last.created_at,
                    trace_id: last.trace_id.clone(),
                });

                (
                    filters.cursor_created_at.is_some(),
                    next_cursor,
                    has_more,
                    previous_cursor,
                )
            }
            _ => (false, None, false, None),
        };

        Ok(TracePaginationResponse {
            items,
            has_next,
            next_cursor,
            has_previous,
            previous_cursor,
        })
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
        service_name: Option<&str>,
    ) -> Result<Vec<TraceSpan>, SqlError> {
        let query = Queries::GetTraceSpans.get_query();
        let trace_items: Result<Vec<TraceSpan>, SqlError> = sqlx::query_as(query)
            .bind(trace_id)
            .bind(service_name)
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
        service_name: Option<&str>,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        bucket_interval_str: &str,
    ) -> Result<Vec<TraceMetricBucket>, SqlError> {
        let query = Queries::GetTraceMetrics.get_query();
        let trace_items: Result<Vec<TraceMetricBucket>, SqlError> = sqlx::query_as(query)
            .bind(service_name)
            .bind(start_time)
            .bind(end_time)
            .bind(bucket_interval_str)
            .fetch_all(pool)
            .await
            .map_err(SqlError::SqlxError);

        trace_items
    }

    async fn refresh_trace_summary(pool: &Pool<Postgres>) -> Result<PgQueryResult, SqlError> {
        let query_result = sqlx::query("REFRESH MATERIALIZED VIEW scouter.trace_summary;")
            .execute(pool)
            .await?;

        Ok(query_result)
    }
}
