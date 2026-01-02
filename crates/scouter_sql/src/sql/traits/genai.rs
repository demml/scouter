use crate::sql::error::SqlError;
use crate::sql::query::Queries;
use crate::sql::schema::BinnedMetricWrapper;
use crate::sql::utils::split_custom_interval;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use itertools::multiunzip;
use scouter_dataframe::parquet::BinnedMetricsExtractor;
use scouter_dataframe::parquet::ParquetDataFrame;
use scouter_settings::ObjectStorageSettings;
use scouter_types::contracts::DriftRequest;
use scouter_types::{
    BinnedMetrics, GenAIDriftRecord, GenAIDriftRecordPaginationRequest,
    GenAIDriftRecordPaginationResponse, RecordCursor, RecordType,
};
use scouter_types::{GenAIDriftInternalRecord, GenAITaskRecord};
use scouter_types::{GenAIMetricRecord, Status};
use sqlx::types::Json;
use sqlx::{postgres::PgQueryResult, Pool, Postgres, Row};
use std::collections::HashMap;
use tracing::error;
use tracing::{debug, instrument};

#[async_trait]
pub trait GenAIDriftSqlLogic {
    /// Inserts an GenAI drift record into the database.
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `record` - The GenAI drift record to insert
    /// # Returns
    /// * A result containing the query result or an error
    async fn insert_genai_event_record(
        pool: &Pool<Postgres>,
        record: &GenAIDriftRecord,
        entity_id: &i32,
    ) -> Result<PgQueryResult, SqlError> {
        let query = Queries::InsertGenAIDriftRecord.get_query();

        sqlx::query(query)
            .bind(&record.uid)
            .bind(record.created_at)
            .bind(entity_id)
            .bind(&record.context)
            .bind(Json(&record.prompt))
            .execute(pool)
            .await
            .map_err(SqlError::SqlxError)
    }

    /// Inserts a batch of GenAI metric values into the database.
    /// This is the output from processing/evaluating the GenAI drift records.
    async fn insert_genai_metric_values_batch(
        pool: &Pool<Postgres>,
        records: &[GenAIMetricRecord],
        entity_id: &i32,
    ) -> Result<PgQueryResult, SqlError> {
        if records.is_empty() {
            return Err(SqlError::EmptyBatchError);
        }

        let query = Queries::InsertGenAIMetricValuesBatch.get_query();

        let (created_ats, record_uids, entity_ids, metrics, values): (
            Vec<DateTime<Utc>>,
            Vec<&str>,
            Vec<&i32>,
            Vec<&str>,
            Vec<f64>,
        ) = multiunzip(records.iter().map(|r| {
            (
                r.created_at,
                r.uid.as_str(),
                entity_id,
                r.metric.as_str(),
                r.value,
            )
        }));

        sqlx::query(query)
            .bind(created_ats)
            .bind(record_uids)
            .bind(entity_ids)
            .bind(metrics)
            .bind(values)
            .execute(pool)
            .await
            .map_err(SqlError::SqlxError)
    }

    async fn get_genai_event_records(
        pool: &Pool<Postgres>,
        limit_datetime: Option<&DateTime<Utc>>,
        status: Option<Status>,
        entity_id: &i32,
    ) -> Result<Vec<GenAIDriftRecord>, SqlError> {
        let mut query_string = Queries::GetGenAIDriftRecords.get_query().to_string();
        let mut bind_count = 1;

        if limit_datetime.is_some() {
            bind_count += 1;
            query_string.push_str(&format!(" AND created_at > ${bind_count}"));
        }

        let status_value = status.as_ref().and_then(|s| s.as_str());
        if status_value.is_some() {
            bind_count += 1;
            query_string.push_str(&format!(" AND status = ${bind_count}"));
        }

        let mut query =
            sqlx::query_as::<_, GenAIDriftInternalRecord>(&query_string).bind(entity_id);

        if let Some(datetime) = limit_datetime {
            query = query.bind(datetime);
        }
        // Bind status if provided
        if let Some(status) = status_value {
            query = query.bind(status);
        }

        let records = query.fetch_all(pool).await.map_err(SqlError::SqlxError)?;

        Ok(records.into_iter().map(|r| r.to_public_record()).collect())
    }

    /// Retrieves a paginated list of GenAI drift records with bidirectional cursor support
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `params` - The pagination request containing limit, cursor, and direction
    /// * `entity_id` - The entity ID to filter records
    ///
    /// # Returns
    /// * Result with paginated response containing GenAI drift records
    #[instrument(skip_all)]
    async fn get_paginated_genai_event_records(
        pool: &Pool<Postgres>,
        params: &GenAIDriftRecordPaginationRequest,
        entity_id: &i32,
    ) -> Result<GenAIDriftRecordPaginationResponse, SqlError> {
        let query = Queries::GetPaginatedGenAIDriftRecords.get_query();
        let limit = params.limit.unwrap_or(50);
        let direction = params.direction.as_deref().unwrap_or("next");

        let mut items: Vec<GenAIDriftInternalRecord> = sqlx::query_as(query)
            .bind(entity_id)
            .bind(params.status.as_ref().and_then(|s| s.as_str()))
            .bind(params.cursor_created_at)
            .bind(direction)
            .bind(params.cursor_id)
            .bind(limit)
            .bind(params.start_datetime)
            .bind(params.end_datetime)
            .fetch_all(pool)
            .await
            .map_err(SqlError::SqlxError)?;

        let has_more = items.len() > limit as usize;

        if has_more {
            items.pop();
        }

        let (has_next, next_cursor, has_previous, previous_cursor) = match direction {
            "previous" => {
                items.reverse();

                let previous_cursor = if has_more {
                    items.first().map(|first| RecordCursor {
                        created_at: first.created_at,
                        id: first.id,
                    })
                } else {
                    None
                };

                let next_cursor = items.last().map(|last| RecordCursor {
                    created_at: last.created_at,
                    id: last.id,
                });

                (
                    params.cursor_created_at.is_some(),
                    next_cursor,
                    has_more,
                    previous_cursor,
                )
            }
            _ => {
                // Forward pagination (default)
                let next_cursor = if has_more {
                    items.last().map(|last| RecordCursor {
                        created_at: last.created_at,
                        id: last.id,
                    })
                } else {
                    None
                };

                let previous_cursor = items.first().map(|first| RecordCursor {
                    created_at: first.created_at,
                    id: first.id,
                });

                (
                    has_more,
                    next_cursor,
                    params.cursor_created_at.is_some(),
                    previous_cursor,
                )
            }
        };

        let public_items = items.into_iter().map(|r| r.to_public_record()).collect();

        Ok(GenAIDriftRecordPaginationResponse {
            items: public_items,
            has_next,
            next_cursor,
            has_previous,
            previous_cursor,
        })
    }

    async fn get_genai_metric_values(
        pool: &Pool<Postgres>,
        limit_datetime: &DateTime<Utc>,
        metrics: &[String],
        entity_id: &i32,
    ) -> Result<HashMap<String, f64>, SqlError> {
        let query = Queries::GetGenAIMetricValues.get_query();

        let records = sqlx::query(query)
            .bind(limit_datetime)
            .bind(entity_id)
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

    // Queries the database for GenAI metric records based on a time window
    /// and aggregation.
    ///
    /// # Arguments
    /// * `pool` - The database connection pool
    /// * `params` - The drift request parameters
    /// * `start_dt` - The start time of the time window
    /// * `end_dt` - The end time of the time window
    /// * `entity_id` - The entity ID to filter records
    ///
    /// # Returns
    /// * BinnedMetrics
    #[instrument(skip_all)]
    async fn get_records(
        pool: &Pool<Postgres>,
        params: &DriftRequest,
        start_dt: DateTime<Utc>,
        end_dt: DateTime<Utc>,
        entity_id: &i32,
    ) -> Result<BinnedMetrics, SqlError> {
        let minutes = end_dt.signed_duration_since(start_dt).num_minutes() as f64;
        let bin = minutes / params.max_data_points as f64;

        let query = Queries::GetBinnedMetrics.get_query();

        let records: Vec<BinnedMetricWrapper> = sqlx::query_as(query)
            .bind(bin)
            .bind(start_dt)
            .bind(end_dt)
            .bind(entity_id)
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
    /// * `entity_id` - The entity ID to filter records
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
        entity_id: &i32,
    ) -> Result<BinnedMetrics, SqlError> {
        debug!("Getting archived GenAI metrics for params: {:?}", params);
        let path = format!("{}/genai_metric", params.uid);
        let bin = minutes as f64 / params.max_data_points as f64;
        let archived_df = ParquetDataFrame::new(storage_settings, &RecordType::GenAIMetric)?
            .get_binned_metrics(&path, &bin, &begin, &end, entity_id)
            .await
            .inspect_err(|e| {
                error!("Failed to get archived GenAI metrics: {:?}", e);
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
    async fn get_binned_genai_metric_values(
        pool: &Pool<Postgres>,
        params: &DriftRequest,
        retention_period: &i32,
        storage_settings: &ObjectStorageSettings,
        entity_id: &i32,
    ) -> Result<BinnedMetrics, SqlError> {
        debug!("Getting binned Custom drift records for {:?}", params);

        if !params.has_custom_interval() {
            debug!("No custom interval provided, using default");
            let (start_dt, end_dt) = params.time_interval.to_begin_end_times()?;
            return Self::get_records(pool, params, start_dt, end_dt, entity_id).await;
        }

        debug!("Custom interval provided, using custom interval");
        let interval = params.clone().to_custom_interval().unwrap();
        let timestamps = split_custom_interval(interval.begin, interval.end, retention_period)?;
        let mut custom_metric_map = BinnedMetrics::default();

        // get data from postgres
        if let Some((active_begin, active_end)) = timestamps.active_range {
            let current_results =
                Self::get_records(pool, params, active_begin, active_end, entity_id).await?;
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
                    entity_id,
                )
                .await?;
                Self::merge_feature_results(archived_results, &mut custom_metric_map)?;
            }
        }

        Ok(custom_metric_map)
    }

    /// Retrieves the next pending GenAI drift task from drift_records.
    async fn get_pending_genai_event_record(
        pool: &Pool<Postgres>,
    ) -> Result<Option<GenAITaskRecord>, SqlError> {
        let query = Queries::GetPendingGenAIDriftTask.get_query();
        let result: Option<GenAITaskRecord> = sqlx::query_as(query)
            .fetch_optional(pool)
            .await
            .map_err(SqlError::SqlxError)?;

        Ok(result)
    }

    #[instrument(skip_all)]
    async fn update_genai_event_record_status(
        pool: &Pool<Postgres>,
        record: &GenAITaskRecord,
        status: Status,
        workflow_duration: Option<i32>, // Duration in seconds
    ) -> Result<(), SqlError> {
        let query = Queries::UpdateGenAIDriftTask.get_query();

        let _query_result = sqlx::query(query)
            .bind(status.as_str())
            .bind(record.score.clone())
            .bind(workflow_duration)
            .bind(&record.uid)
            .execute(pool)
            .await
            .inspect_err(|e| {
                error!("Failed to update GenAI drift record status: {:?}", e);
            })?;

        Ok(())
    }
}
