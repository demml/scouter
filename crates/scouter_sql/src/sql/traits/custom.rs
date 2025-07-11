use crate::sql::error::SqlError;
use crate::sql::query::Queries;
use crate::sql::schema::BinnedCustomMetricWrapper;
use crate::sql::utils::split_custom_interval;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use scouter_dataframe::parquet::{dataframe_to_custom_drift_metrics, ParquetDataFrame};
use scouter_settings::ObjectStorageSettings;
use scouter_types::contracts::{DriftRequest, ServiceInfo};
use scouter_types::{custom::BinnedCustomMetrics, CustomMetricServerRecord, RecordType};
use sqlx::{postgres::PgQueryResult, Pool, Postgres, Row};
use std::collections::HashMap;
use tracing::{debug, instrument};

#[async_trait]
pub trait CustomMetricSqlLogic {
    async fn insert_custom_metric_values_batch(
        pool: &Pool<Postgres>,
        records: &[CustomMetricServerRecord],
    ) -> Result<PgQueryResult, SqlError> {
        if records.is_empty() {
            return Err(SqlError::EmptyBatchError);
        }

        let query = Queries::InsertCustomMetricValuesBatch.get_query();

        let created_ats: Vec<DateTime<Utc>> = records.iter().map(|r| r.created_at).collect();
        let names: Vec<&str> = records.iter().map(|r| r.name.as_str()).collect();
        let spaces: Vec<&str> = records.iter().map(|r| r.space.as_str()).collect();
        let versions: Vec<&str> = records.iter().map(|r| r.version.as_str()).collect();
        let metrics: Vec<&str> = records.iter().map(|r| r.metric.as_str()).collect();
        let values: Vec<f64> = records.iter().map(|r| r.value).collect();

        sqlx::query(&query.sql)
            .bind(created_ats)
            .bind(names)
            .bind(spaces)
            .bind(versions)
            .bind(metrics)
            .bind(values)
            .execute(pool)
            .await
            .map_err(SqlError::SqlxError)
    }

    async fn get_custom_metric_values(
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
            .map_err(SqlError::SqlxError)?;

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

    // Queries the database for Custom drift records based on a time window
    /// and aggregation.
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `params` - The drift request parameters
    ///
    /// # Returns
    /// * BinnedCustomMetrics
    #[instrument(skip_all)]
    async fn get_records(
        pool: &Pool<Postgres>,
        params: &DriftRequest,
        minutes: i32,
    ) -> Result<BinnedCustomMetrics, SqlError> {
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
            .map_err(SqlError::SqlxError)?;

        Ok(BinnedCustomMetrics::from_vec(
            records.into_iter().map(|wrapper| wrapper.0).collect(),
        ))
    }

    /// Helper for merging custom drift records
    fn merge_feature_results(
        results: BinnedCustomMetrics,
        map: &mut BinnedCustomMetrics,
    ) -> Result<(), SqlError> {
        for (name, metric) in results.metrics {
            let metric_clone = metric.clone();
            map.metrics
                .entry(name)
                .and_modify(|existing| {
                    existing.created_at.extend(metric_clone.created_at);
                    existing.stats.extend(metric_clone.stats);
                })
                .or_insert(metric);
        }

        Ok(())
    }

    /// DataFusion implementation for getting custom drift records from archived data.
    ///
    /// # Arguments
    /// * `params` - The drift request parameters
    /// * `begin` - The start time of the time window
    /// * `end` - The end time of the time window
    /// * `minutes` - The number of minutes to bin the data
    /// * `storage_settings` - The object storage settings
    ///
    /// # Returns
    /// * A vector of drift records
    #[instrument(skip_all)]
    async fn get_archived_records(
        params: &DriftRequest,
        begin: DateTime<Utc>,
        end: DateTime<Utc>,
        minutes: i32,
        storage_settings: &ObjectStorageSettings,
    ) -> Result<BinnedCustomMetrics, SqlError> {
        let path = format!("{}/{}/{}/custom", params.space, params.name, params.version);
        let bin = minutes as f64 / params.max_data_points as f64;
        let archived_df = ParquetDataFrame::new(storage_settings, &RecordType::Custom)?
            .get_binned_metrics(
                &path,
                &bin,
                &begin,
                &end,
                &params.space,
                &params.name,
                &params.version,
            )
            .await?;

        Ok(dataframe_to_custom_drift_metrics(archived_df).await?)
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
    #[instrument(skip_all)]
    async fn get_binned_custom_drift_records(
        pool: &Pool<Postgres>,
        params: &DriftRequest,
        retention_period: &i32,
        storage_settings: &ObjectStorageSettings,
    ) -> Result<BinnedCustomMetrics, SqlError> {
        debug!("Getting binned Custom drift records for {:?}", params);

        if !params.has_custom_interval() {
            debug!("No custom interval provided, using default");
            let minutes = params.time_interval.to_minutes();
            return Self::get_records(pool, params, minutes).await;
        }

        debug!("Custom interval provided, using custom interval");
        let interval = params.clone().to_custom_interval().unwrap();
        let timestamps = split_custom_interval(interval.start, interval.end, retention_period)?;
        let mut custom_metric_map = BinnedCustomMetrics::default();

        // get data from postgres
        if let Some(minutes) = timestamps.current_minutes {
            let current_results = Self::get_records(pool, params, minutes).await?;
            Self::merge_feature_results(current_results, &mut custom_metric_map)?;
        }

        // get archived data

        if let Some((archive_begin, archive_end)) = timestamps.archived_range {
            if let Some(archived_minutes) = timestamps.archived_minutes {
                let archived_results = Self::get_archived_records(
                    params,
                    archive_begin,
                    archive_end,
                    archived_minutes,
                    storage_settings,
                )
                .await?;
                Self::merge_feature_results(archived_results, &mut custom_metric_map)?;
            }
        }

        Ok(custom_metric_map)
    }
}
