use crate::sql::query::Queries;
use crate::sql::schema::BinnedCustomMetricWrapper;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use scouter_contracts::{DriftRequest, ServiceInfo};
use scouter_error::SqlError;
use scouter_types::{custom::BinnedCustomMetrics, CustomMetricServerRecord};
use sqlx::{postgres::PgQueryResult, Pool, Postgres, Row};
use std::collections::HashMap;
#[async_trait]
pub trait CustomMetricSqlLogic {
    /// Inserts a custom metric value into the database.
    async fn insert_custom_metric_value(
        &self,
        pool: &Pool<Postgres>,
        record: &CustomMetricServerRecord,
    ) -> Result<PgQueryResult, SqlError> {
        let query = Queries::InsertCustomMetricValues.get_query();

        let query_result = sqlx::query(&query.sql)
            .bind(record.created_at)
            .bind(&record.name)
            .bind(&record.space)
            .bind(&record.version)
            .bind(&record.metric)
            .bind(record.value)
            .execute(pool)
            .await;

        match query_result {
            Ok(result) => Ok(result),
            Err(e) => Err(SqlError::traced_insert_custom_metrics_error(e)),
        }
    }

    async fn get_custom_metric_values(
        &self,
        pool: &Pool<Postgres>,
        service_info: &ServiceInfo,
        limit_datetime: &DateTime<Utc>,
        metrics: &[String],
    ) -> Result<HashMap<String, f64>, SqlError> {
        let query = Queries::GetCustomMetricValues.get_query();

        let records = sqlx::query(&query.sql)
            .bind(&service_info.name)
            .bind(&service_info.space)
            .bind(&service_info.version)
            .bind(limit_datetime)
            .bind(metrics)
            .fetch_all(pool)
            .await
            .map_err(SqlError::traced_get_custom_metrics_error)?;

        let metric_map = records
            .into_iter()
            .map(|row| {
                let metric = row.get("metric");
                let value = row.get("value");
                (metric, value)
            })
            .collect();

        Ok(metric_map)
    }

    // Queries the database for drift records based on a time window and aggregation
    //
    // # Arguments
    //
    // * `name` - The name of the service to query drift records for
    // * `params` - The drift request parameters
    // # Returns
    //
    // * A vector of drift records
    async fn get_binned_custom_drift_records(
        &self,
        pool: &Pool<Postgres>,
        params: &DriftRequest,
    ) -> Result<BinnedCustomMetrics, SqlError> {
        // get features

        let minutes = params.time_interval.to_minutes();
        let bin = params.time_interval.to_minutes() as f64 / params.max_data_points as f64;

        let query = Queries::GetBinnedCustomMetricValues.get_query();

        let records: Vec<BinnedCustomMetricWrapper> = sqlx::query_as(&query.sql)
            .bind(bin)
            .bind(minutes)
            .bind(&params.name)
            .bind(&params.space)
            .bind(&params.version)
            .fetch_all(pool)
            .await
            .map_err(SqlError::traced_query_error)?;

        Ok(BinnedCustomMetrics::from_vec(
            records.into_iter().map(|wrapper| wrapper.0).collect(),
        ))
    }
}
