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

    async fn insert_span_batch(pool: &Pool<Postgres>, spans: &Vec<TraceSpanRecord>) {}
}
