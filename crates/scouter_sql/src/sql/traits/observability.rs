use crate::sql::query::Queries;
use crate::sql::schema::ObservabilityResult;

use crate::sql::error::SqlError;
use scouter_types::TimeInterval;
use scouter_types::{ObservabilityMetricRequest, ObservabilityMetrics};

use sqlx::{postgres::PgQueryResult, Pool, Postgres};

use async_trait::async_trait;

#[async_trait]
pub trait ObservabilitySqlLogic {
    // Inserts a drift record into the database
    //
    // # Arguments
    //
    // * `record` - A drift record to insert into the database
    // * `entity_id` - entity associated with observability records
    //
    async fn insert_observability_record(
        pool: &Pool<Postgres>,
        record: &ObservabilityMetrics,
        entity_id: &i32,
    ) -> Result<PgQueryResult, SqlError> {
        let query = Queries::InsertObservabilityRecord.get_query();
        let route_metrics = serde_json::to_value(&record.route_metrics)?;

        sqlx::query(query)
            .bind(entity_id)
            .bind(record.request_count)
            .bind(record.error_count)
            .bind(route_metrics)
            .execute(pool)
            .await
            .map_err(SqlError::SqlxError)
    }

    async fn get_binned_observability_metrics(
        pool: &Pool<Postgres>,
        params: &ObservabilityMetricRequest,
        entity_id: &i32,
    ) -> Result<Vec<ObservabilityResult>, SqlError> {
        let query = Queries::GetBinnedObservabilityMetrics.get_query();

        let time_interval = TimeInterval::from_string(&params.time_interval).to_minutes();

        let bin = time_interval as f64 / params.max_data_points as f64;

        sqlx::query_as(query)
            .bind(bin)
            .bind(time_interval)
            .bind(entity_id)
            .fetch_all(pool)
            .await
            .map_err(SqlError::SqlxError)
    }
}
