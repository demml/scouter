use crate::sql::error::SqlError;
use crate::sql::query::Queries;
use crate::sql::schema::LLMDriftTaskRequest;
use crate::sql::schema::{BinnedMetricWrapper, LLMDriftServerSQLRecord};
use crate::sql::utils::split_custom_interval;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use itertools::multiunzip;
use scouter_dataframe::parquet::BinnedMetricsExtractor;
use scouter_dataframe::parquet::ParquetDataFrame;
use scouter_settings::ObjectStorageSettings;
use scouter_types::contracts::{DriftRequest, ServiceInfo};
use scouter_types::{
    llm::{PaginationCursor, PaginationRequest, PaginationResponse},
    BinnedMetrics, LLMDriftServerRecord, RecordType,
};
use scouter_types::{LLMMetricServerRecord, Status};
use sqlx::{postgres::PgQueryResult, Pool, Postgres, Row};
use std::collections::HashMap;
use std::str::FromStr;
use tracing::error;
use tracing::{debug, instrument};

#[async_trait]
pub trait LLMDriftSqlLogic {
    /// Inserts an LLM drift record into the database.
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `record` - The LLM drift record to insert
    /// # Returns
    /// * A result containing the query result or an error
    async fn insert_llm_drift_record(
        pool: &Pool<Postgres>,
        record: &LLMDriftServerRecord,
    ) -> Result<PgQueryResult, SqlError> {
        let query = Queries::InsertLLMDriftRecord.get_query();

        let sql_record = LLMDriftServerSQLRecord::from_server_record(record);

        sqlx::query(&query.sql)
            .bind(sql_record.created_at)
            .bind(&sql_record.space)
            .bind(&sql_record.name)
            .bind(&sql_record.version)
            .bind(&sql_record.input)
            .bind(&sql_record.response)
            .bind(&sql_record.context)
            .bind(&sql_record.prompt)
            .execute(pool)
            .await
            .map_err(SqlError::SqlxError)
    }

    /// Inserts a batch of LLM metric values into the database.
    /// This is the output from processing/evaluating the LLM drift records.
    async fn insert_llm_metric_values_batch(
        pool: &Pool<Postgres>,
        records: &[LLMMetricServerRecord],
    ) -> Result<PgQueryResult, SqlError> {
        if records.is_empty() {
            return Err(SqlError::EmptyBatchError);
        }

        let query = Queries::InsertLLMMetricValuesBatch.get_query();

        let (created_ats, names, spaces, versions, metrics, values): (
            Vec<DateTime<Utc>>,
            Vec<&str>,
            Vec<&str>,
            Vec<&str>,
            Vec<&str>,
            Vec<f64>,
        ) = multiunzip(records.iter().map(|r| {
            (
                r.created_at,
                r.name.as_str(),
                r.space.as_str(),
                r.version.as_str(),
                r.metric.as_str(),
                r.value,
            )
        }));

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

    async fn get_llm_drift_records(
        pool: &Pool<Postgres>,
        service_info: &ServiceInfo,
        limit_datetime: Option<&DateTime<Utc>>,
        status: Option<Status>,
    ) -> Result<Vec<LLMDriftServerRecord>, SqlError> {
        let mut query_string = Queries::GetLLMDriftRecords.get_query().sql;

        let mut bind_count = 3;

        if limit_datetime.is_some() {
            bind_count += 1;
            query_string.push_str(&format!(" AND created_at > ${bind_count}"));
        }

        let status_value = status.as_ref().and_then(|s| s.as_str());
        if status_value.is_some() {
            bind_count += 1;
            query_string.push_str(&format!(" AND status = ${bind_count}"));
        }

        let mut query = sqlx::query_as::<_, LLMDriftServerSQLRecord>(&query_string)
            .bind(&service_info.space)
            .bind(&service_info.name)
            .bind(&service_info.version);

        if let Some(datetime) = limit_datetime {
            query = query.bind(datetime);
        }
        // Bind status if provided
        if let Some(status) = status_value {
            query = query.bind(status);
        }

        let records = query.fetch_all(pool).await.map_err(SqlError::SqlxError)?;

        Ok(records
            .into_iter()
            .map({
                |record| LLMDriftServerRecord {
                    created_at: record.created_at,
                    space: record.space,
                    name: record.name,
                    version: record.version,
                    input: record.input,
                    response: record.response,
                    prompt: record.prompt.0,
                    context: record.context.0,
                    status: Status::from_str(&record.status).unwrap_or(Status::Pending), // Default to Pending if parsing fails
                    id: record.id,                 // Ensure we include the ID
                    uid: record.uid,               // Include the UID
                    updated_at: record.updated_at, // Include the updated_at field
                    processing_started_at: record.processing_started_at, // Include the processing_started_at field
                    processing_ended_at: record.processing_ended_at, // Include the processing_ended_at
                }
            })
            .collect())
    }

    /// Retrieves a paginated list of LLM drift records from the database
    /// for a given service.
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `service_info` - The service information to filter records by
    /// * `status` - Optional status filter for the records
    /// * `pagination` - The pagination request containing limit and cursor
    /// # Returns
    /// * A result containing a pagination response with LLM drift records or an error
    #[instrument(skip_all)]
    async fn get_llm_drift_records_pagination(
        pool: &Pool<Postgres>,
        service_info: &ServiceInfo,
        status: Option<Status>,
        pagination: PaginationRequest,
    ) -> Result<PaginationResponse<LLMDriftServerRecord>, SqlError> {
        let limit = pagination.limit.clamp(1, 100); // Cap at 100, min 1
        let query_limit = limit + 1;

        // Get initial SQL query
        let mut sql = Queries::GetLLMDriftRecords.get_query().sql;
        let mut bind_count = 3;

        // If querying any page other than the first, we need to add a cursor condition
        // Everything is filtered by ID desc (most recent), so if last ID is provided, we need to filter for IDs less than that
        if pagination.cursor.is_some() {
            bind_count += 1;
            sql.push_str(&format!(" AND id < ${bind_count}"));
        }

        // Optional status filter
        let status_value = status.as_ref().and_then(|s| s.as_str());
        if status_value.is_some() {
            bind_count += 1;
            sql.push_str(&format!(" AND status = ${bind_count}"));
        }

        sql.push_str(&format!(" ORDER BY id DESC LIMIT ${}", bind_count + 1));

        let mut query = sqlx::query_as::<_, LLMDriftServerSQLRecord>(&sql)
            .bind(&service_info.space)
            .bind(&service_info.name)
            .bind(&service_info.version);

        // Bind cursor parameter
        if let Some(cursor) = &pagination.cursor {
            query = query.bind(cursor.id);
        }

        // Bind status if provided
        if let Some(status) = status_value {
            query = query.bind(status);
        }

        // Bind limit
        query = query.bind(query_limit);

        let mut records = query.fetch_all(pool).await.map_err(SqlError::SqlxError)?;

        // Check if there are more records
        let has_more = records.len() > limit as usize;
        if has_more {
            records.pop(); // Remove the extra record
        }

        let next_cursor = if has_more && !records.is_empty() {
            let last_record = records.last().unwrap();
            Some(PaginationCursor { id: last_record.id })
        } else {
            None
        };

        let items = records
            .into_iter()
            .map(|record| LLMDriftServerRecord {
                created_at: record.created_at,
                space: record.space,
                name: record.name,
                version: record.version,
                input: record.input,
                response: record.response,
                prompt: record.prompt.0,
                context: record.context.0,
                status: Status::from_str(&record.status).unwrap_or(Status::Pending),
                id: record.id,                 // Ensure we include the ID
                uid: record.uid,               // Include the UID
                updated_at: record.updated_at, // Include the updated_at field
                processing_started_at: record.processing_started_at, // Include the processing_started_at field
                processing_ended_at: record.processing_ended_at, // Include the processing_ended_at
            })
            .collect();

        Ok(PaginationResponse {
            items,
            next_cursor,
            has_more,
        })
    }

    async fn get_llm_metric_values(
        pool: &Pool<Postgres>,
        service_info: &ServiceInfo,
        limit_datetime: &DateTime<Utc>,
        metrics: &[String],
    ) -> Result<HashMap<String, f64>, SqlError> {
        let query = Queries::GetLLMMetricValues.get_query();

        let records = sqlx::query(&query.sql)
            .bind(limit_datetime)
            .bind(&service_info.space)
            .bind(&service_info.name)
            .bind(&service_info.version)
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

    // Queries the database for LLM metric records based on a time window
    /// and aggregation.
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `params` - The drift request parameters
    ///
    /// # Returns
    /// * BinnedMetrics
    #[instrument(skip_all)]
    async fn get_records(
        pool: &Pool<Postgres>,
        params: &DriftRequest,
        minutes: i32,
    ) -> Result<BinnedMetrics, SqlError> {
        let bin = params.time_interval.to_minutes() as f64 / params.max_data_points as f64;

        let query = Queries::GetBinnedMetrics.get_query();

        let records: Vec<BinnedMetricWrapper> = sqlx::query_as(&query.sql)
            .bind(bin)
            .bind(minutes)
            .bind(&params.space)
            .bind(&params.name)
            .bind(&params.version)
            .fetch_all(pool)
            .await
            .map_err(SqlError::SqlxError)?;

        Ok(BinnedMetrics::from_vec(
            records.into_iter().map(|wrapper| wrapper.0).collect(),
        ))
    }

    /// Helper for merging custom drift records
    fn merge_feature_results(
        results: BinnedMetrics,
        map: &mut BinnedMetrics,
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
    async fn get_archived_metric_records(
        params: &DriftRequest,
        begin: DateTime<Utc>,
        end: DateTime<Utc>,
        minutes: i32,
        storage_settings: &ObjectStorageSettings,
    ) -> Result<BinnedMetrics, SqlError> {
        debug!("Getting archived LLM metrics for params: {:?}", params);
        let path = format!(
            "{}/{}/{}/llm_metric",
            params.space, params.name, params.version
        );
        let bin = minutes as f64 / params.max_data_points as f64;
        let archived_df = ParquetDataFrame::new(storage_settings, &RecordType::LLMMetric)?
            .get_binned_metrics(
                &path,
                &bin,
                &begin,
                &end,
                &params.space,
                &params.name,
                &params.version,
            )
            .await
            .inspect_err(|e| {
                error!("Failed to get archived LLM metrics: {:?}", e);
            })?;

        Ok(BinnedMetricsExtractor::dataframe_to_binned_metrics(archived_df).await?)
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
    async fn get_binned_llm_metric_values(
        pool: &Pool<Postgres>,
        params: &DriftRequest,
        retention_period: &i32,
        storage_settings: &ObjectStorageSettings,
    ) -> Result<BinnedMetrics, SqlError> {
        debug!("Getting binned Custom drift records for {:?}", params);

        if !params.has_custom_interval() {
            debug!("No custom interval provided, using default");
            let minutes = params.time_interval.to_minutes();
            return Self::get_records(pool, params, minutes).await;
        }

        debug!("Custom interval provided, using custom interval");
        let interval = params.clone().to_custom_interval().unwrap();
        let timestamps = split_custom_interval(interval.start, interval.end, retention_period)?;
        let mut custom_metric_map = BinnedMetrics::default();

        // get data from postgres
        if let Some(minutes) = timestamps.current_minutes {
            let current_results = Self::get_records(pool, params, minutes).await?;
            Self::merge_feature_results(current_results, &mut custom_metric_map)?;
        }

        // get archived data
        if let Some((archive_begin, archive_end)) = timestamps.archived_range {
            if let Some(archived_minutes) = timestamps.archived_minutes {
                let archived_results = Self::get_archived_metric_records(
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

    /// Retrieves the next pending LLM drift task from drift_records.
    async fn get_pending_llm_drift_task(
        pool: &Pool<Postgres>,
    ) -> Result<Option<LLMDriftTaskRequest>, SqlError> {
        let query = Queries::GetPendingLLMDriftTask.get_query();
        sqlx::query_as(&query.sql)
            .fetch_optional(pool)
            .await
            .map_err(SqlError::SqlxError)
    }

    #[instrument(skip_all)]
    async fn update_llm_drift_task_status(
        pool: &Pool<Postgres>,
        task_info: &LLMDriftTaskRequest,
        status: Status,
    ) -> Result<(), SqlError> {
        let query = Queries::UpdateLLMDriftTask.get_query();

        let _query_result = sqlx::query(&query.sql)
            .bind(status.as_str())
            .bind(&task_info.uid)
            .execute(pool)
            .await
            .map_err(SqlError::SqlxError)?;

        Ok(())
    }
}
