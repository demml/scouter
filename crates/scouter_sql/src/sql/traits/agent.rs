use crate::sql::error::SqlError;
use crate::sql::query::Queries;
use crate::sql::schema::BinnedMetricWrapper;
use crate::sql::utils::split_custom_interval;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use scouter_dataframe::parquet::BinnedMetricsExtractor;
use scouter_dataframe::parquet::ParquetDataFrame;
use scouter_settings::ObjectStorageSettings;
use scouter_types::AgentEvalWorkflowPaginationResponse;
use scouter_types::AgentEvalWorkflowResult;
use scouter_types::BoxedEvalRecord;
use scouter_types::EvalRecord;
use scouter_types::EvalTaskResult;
use scouter_types::Status;
use scouter_types::contracts::DriftRequest;
use scouter_types::{
    BinnedMetrics, EvalRecordPaginationRequest, EvalRecordPaginationResponse, EvalRecordSource,
    RecordCursor, RecordType, TraceId,
};
use sqlx::types::Json;
use sqlx::{Pool, Postgres, Row, Transaction, postgres::PgQueryResult};
use std::collections::HashMap;
use tracing::error;
use tracing::{debug, instrument};

#[async_trait]
pub trait AgentDriftSqlLogic {
    async fn insert_agent_eval_record(
        pool: &Pool<Postgres>,
        record: BoxedEvalRecord,
        entity_id: &i32,
        status: Status,
    ) -> Result<PgQueryResult, SqlError> {
        Self::insert_agent_eval_record_with_ready_delay(
            pool,
            record,
            entity_id,
            status,
            Duration::zero(),
        )
        .await
    }

    async fn insert_agent_eval_record_with_ready_delay(
        pool: &Pool<Postgres>,
        record: BoxedEvalRecord,
        entity_id: &i32,
        status: Status,
        ready_delay: Duration,
    ) -> Result<PgQueryResult, SqlError> {
        let query = Queries::InsertEvalRecord.get_query();
        let ready_at = Utc::now() + ready_delay;

        sqlx::query(query)
            .bind(record.record.uid)
            .bind(record.record.created_at)
            .bind(entity_id)
            .bind(Json(record.record.context))
            .bind(&record.record.record_id)
            .bind(&record.record.session_id)
            .bind(record.record.trace_id.map(|t| t.as_bytes().to_vec()))
            .bind(EvalRecordSource::Queue.as_str())
            .bind(&record.record.tags)
            .bind(Json(&record.record.media))
            .bind(record.record.span_id.map(|s| s.as_bytes().to_vec()))
            .bind(status.as_str())
            .bind(ready_at)
            .execute(pool)
            .await
            .map_err(SqlError::SqlxError)
    }

    async fn insert_trace_commit_events(
        pool: &Pool<Postgres>,
        trace_ids: &[TraceId],
    ) -> Result<PgQueryResult, SqlError> {
        if trace_ids.is_empty() {
            return Ok(PgQueryResult::default());
        }

        let bytes: Vec<Vec<u8>> = trace_ids.iter().map(|t| t.as_bytes().to_vec()).collect();
        sqlx::query(Queries::InsertTraceCommitEvents.get_query())
            .bind(bytes)
            .execute(pool)
            .await
            .map_err(SqlError::SqlxError)
    }

    async fn trace_commit_event_exists(
        pool: &Pool<Postgres>,
        trace_id: &TraceId,
    ) -> Result<bool, SqlError> {
        let row = sqlx::query(Queries::TraceCommitEventExists.get_query())
            .bind(trace_id.as_bytes().to_vec())
            .fetch_one(pool)
            .await
            .map_err(SqlError::SqlxError)?;
        Ok(row.try_get::<bool, _>("exists")?)
    }

    async fn claim_trace_commit_events(
        tx: &mut Transaction<'_, Postgres>,
        limit: i64,
    ) -> Result<Vec<(i64, TraceId)>, SqlError> {
        let rows = sqlx::query(Queries::ClaimTraceCommitEvents.get_query())
            .bind(limit)
            .fetch_all(&mut **tx)
            .await
            .map_err(SqlError::SqlxError)?;

        rows.into_iter()
            .map(|row| {
                let id: i64 = row.try_get("id")?;
                let bytes: Vec<u8> = row.try_get("trace_id")?;
                let trace_id = TraceId::from_slice(&bytes)
                    .map_err(|e| SqlError::InvalidInboxData(format!("invalid trace_id: {e}")))?;
                Ok((id, trace_id))
            })
            .collect()
    }

    async fn flip_awaiting_evals(
        tx: &mut Transaction<'_, Postgres>,
        trace_ids: &[TraceId],
        ready_delay: chrono::Duration,
    ) -> Result<PgQueryResult, SqlError> {
        let bytes: Vec<Vec<u8>> = trace_ids.iter().map(|t| t.as_bytes().to_vec()).collect();
        let pg_interval = sqlx::postgres::types::PgInterval {
            months: 0,
            days: 0,
            microseconds: ready_delay.num_microseconds().unwrap_or(0),
        };
        sqlx::query(Queries::FlipAwaitingEvals.get_query())
            .bind(bytes)
            .bind(pg_interval)
            .execute(&mut **tx)
            .await
            .map_err(SqlError::SqlxError)
    }

    async fn mark_events_processed(
        tx: &mut Transaction<'_, Postgres>,
        event_ids: &[i64],
    ) -> Result<PgQueryResult, SqlError> {
        sqlx::query(Queries::MarkEventsProcessed.get_query())
            .bind(event_ids)
            .execute(&mut **tx)
            .await
            .map_err(SqlError::SqlxError)
    }

    async fn sweep_awaiting_trace_timeouts(
        pool: &Pool<Postgres>,
        timeout: chrono::Duration,
    ) -> Result<PgQueryResult, SqlError> {
        let pg_interval = sqlx::postgres::types::PgInterval {
            months: 0,
            days: 0,
            microseconds: timeout.num_microseconds().unwrap_or(0),
        };
        sqlx::query(Queries::SweepAwaitingTraceTimeouts.get_query())
            .bind(pg_interval)
            .execute(pool)
            .await
            .map_err(SqlError::SqlxError)
    }

    async fn prune_processed_events(
        pool: &Pool<Postgres>,
        retention: chrono::Duration,
    ) -> Result<PgQueryResult, SqlError> {
        let pg_interval = sqlx::postgres::types::PgInterval {
            months: 0,
            days: 0,
            microseconds: retention.num_microseconds().unwrap_or(0),
        };
        sqlx::query(Queries::PruneProcessedEvents.get_query())
            .bind(pg_interval)
            .execute(pool)
            .await
            .map_err(SqlError::SqlxError)
    }

    async fn insert_agent_eval_workflow_record(
        pool: &Pool<Postgres>,
        record: &AgentEvalWorkflowResult,
        entity_id: &i32,
    ) -> Result<PgQueryResult, SqlError> {
        let query = Queries::InsertAgentWorkflowResult.get_query();

        sqlx::query(query)
            .bind(record.created_at)
            .bind(record.record_uid.as_str())
            .bind(entity_id)
            .bind(record.total_tasks)
            .bind(record.passed_tasks)
            .bind(record.failed_tasks)
            .bind(record.pass_rate)
            .bind(record.duration_ms)
            .bind(Json(&record.execution_plan))
            .execute(pool)
            .await
            .map_err(SqlError::SqlxError)
    }

    async fn insert_eval_task_results_batch(
        pool: &Pool<Postgres>,
        records: &[EvalTaskResult],
        entity_id: &i32,
    ) -> Result<sqlx::postgres::PgQueryResult, SqlError> {
        if records.is_empty() {
            return Err(SqlError::EmptyBatchError);
        }

        let n = records.len();

        let mut created_ats = Vec::with_capacity(n);
        let mut start_times = Vec::with_capacity(n);
        let mut end_times = Vec::with_capacity(n);
        let mut record_uids = Vec::with_capacity(n);
        let mut entity_ids = Vec::with_capacity(n);
        let mut task_ids = Vec::with_capacity(n);
        let mut task_types = Vec::with_capacity(n);
        let mut passed_flags = Vec::with_capacity(n);
        let mut values = Vec::with_capacity(n);
        let mut assertions = Vec::with_capacity(n);
        let mut operators = Vec::with_capacity(n);
        let mut expected_jsons = Vec::with_capacity(n);
        let mut actual_jsons = Vec::with_capacity(n);
        let mut messages = Vec::with_capacity(n);
        let mut condition = Vec::with_capacity(n);
        let mut stage = Vec::with_capacity(n);

        for r in records {
            created_ats.push(r.created_at);
            start_times.push(r.start_time);
            end_times.push(r.end_time);
            record_uids.push(&r.record_uid);
            entity_ids.push(entity_id);
            task_ids.push(&r.task_id);
            task_types.push(r.task_type.as_str());
            passed_flags.push(r.passed);
            values.push(r.value);
            assertions.push(Json(r.assertion()));
            operators.push(r.operator.as_str());
            expected_jsons.push(Json(&r.expected));
            actual_jsons.push(Json(&r.actual));
            messages.push(&r.message);
            condition.push(r.condition);
            stage.push(r.stage);
        }

        let query = Queries::InsertAgentTaskResultsBatch.get_query();

        sqlx::query(query)
            .bind(&created_ats)
            .bind(&start_times)
            .bind(&end_times)
            .bind(&record_uids)
            .bind(&entity_ids)
            .bind(&task_ids)
            .bind(&task_types)
            .bind(&passed_flags)
            .bind(&values)
            .bind(&assertions)
            .bind(&operators)
            .bind(&expected_jsons)
            .bind(&actual_jsons)
            .bind(&messages)
            .bind(&condition)
            .bind(&stage)
            .execute(pool)
            .await
            .map_err(SqlError::SqlxError)
    }

    async fn get_agent_eval_records(
        pool: &Pool<Postgres>,
        limit_datetime: Option<&DateTime<Utc>>,
        status: Option<Status>,
        entity_id: &i32,
    ) -> Result<Vec<EvalRecord>, SqlError> {
        let mut query_string = Queries::GetEvalRecords.get_query().to_string();
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

        let mut query = sqlx::query_as::<_, EvalRecord>(&query_string).bind(entity_id);

        if let Some(datetime) = limit_datetime {
            query = query.bind(datetime);
        }
        if let Some(status) = status_value {
            query = query.bind(status);
        }

        let records = query.fetch_all(pool).await.map_err(SqlError::SqlxError)?;

        Ok(records
            .into_iter()
            .map(|mut r| {
                r.mask_sensitive_data();
                r
            })
            .collect())
    }

    #[instrument(skip_all)]
    async fn get_paginated_agent_eval_records(
        pool: &Pool<Postgres>,
        params: &EvalRecordPaginationRequest,
        entity_id: &i32,
    ) -> Result<EvalRecordPaginationResponse, SqlError> {
        let query = Queries::GetPaginatedEvalRecords.get_query();
        let limit = params.limit.unwrap_or(50);
        let direction = params.direction.as_deref().unwrap_or("next");

        let mut items: Vec<EvalRecord> = sqlx::query_as(query)
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

        let public_items = items
            .into_iter()
            .map(|mut r| {
                r.mask_sensitive_data();
                r
            })
            .collect();

        Ok(EvalRecordPaginationResponse {
            items: public_items,
            has_next,
            next_cursor,
            has_previous,
            previous_cursor,
        })
    }

    async fn get_agent_eval_task(
        pool: &Pool<Postgres>,
        record_uid: &str,
    ) -> Result<Vec<EvalTaskResult>, SqlError> {
        let query = Queries::GetAgentEvalTasks.get_query();
        sqlx::query_as(query)
            .bind(record_uid)
            .fetch_all(pool)
            .await
            .map_err(SqlError::SqlxError)
    }

    #[instrument(skip_all)]
    async fn get_paginated_agent_eval_workflow_records(
        pool: &Pool<Postgres>,
        params: &EvalRecordPaginationRequest,
        entity_id: &i32,
    ) -> Result<AgentEvalWorkflowPaginationResponse, SqlError> {
        let query = Queries::GetPaginatedAgentEvalWorkflow.get_query();
        let limit = params.limit.unwrap_or(50);
        let direction = params.direction.as_deref().unwrap_or("next");

        let mut items: Vec<AgentEvalWorkflowResult> = sqlx::query_as(query)
            .bind(entity_id)
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

        let public_items = items
            .into_iter()
            .map(|mut r| {
                r.mask_sensitive_data();
                r
            })
            .collect();

        Ok(AgentEvalWorkflowPaginationResponse {
            items: public_items,
            has_next,
            next_cursor,
            has_previous,
            previous_cursor,
        })
    }

    #[instrument(skip_all)]
    async fn get_agent_task_values(
        pool: &Pool<Postgres>,
        limit_datetime: &DateTime<Utc>,
        metrics: &[String],
        entity_id: &i32,
    ) -> Result<HashMap<String, f64>, SqlError> {
        let query = Queries::GetAgentTaskValues.get_query();

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

    #[instrument(skip_all)]
    async fn get_agent_workflow_value(
        pool: &Pool<Postgres>,
        limit_datetime: &DateTime<Utc>,
        entity_id: &i32,
    ) -> Result<Option<f64>, SqlError> {
        let query = Queries::GetAgentWorkflowValues.get_query();

        let records = sqlx::query(query)
            .bind(limit_datetime)
            .bind(entity_id)
            .fetch_optional(pool)
            .await
            .inspect_err(|e| {
                error!("Error fetching agent workflow values: {:?}", e);
            })?;

        Ok(records.and_then(|r| r.try_get("value").ok()))
    }

    #[instrument(skip_all)]
    async fn get_binned_workflow_records(
        pool: &Pool<Postgres>,
        params: &DriftRequest,
        start_dt: DateTime<Utc>,
        end_dt: DateTime<Utc>,
        entity_id: &i32,
    ) -> Result<BinnedMetrics, SqlError> {
        let minutes = end_dt.signed_duration_since(start_dt).num_minutes() as f64;
        let bin = minutes / params.max_data_points as f64;

        let query = Queries::GetAgentWorkflowBinnedMetrics.get_query();

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

    #[instrument(skip_all)]
    async fn get_binned_task_records(
        pool: &Pool<Postgres>,
        params: &DriftRequest,
        start_dt: DateTime<Utc>,
        end_dt: DateTime<Utc>,
        entity_id: &i32,
    ) -> Result<BinnedMetrics, SqlError> {
        let minutes = end_dt.signed_duration_since(start_dt).num_minutes() as f64;
        let bin = minutes / params.max_data_points as f64;

        let query = Queries::GetAgentTaskBinnedMetrics.get_query();

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

    #[instrument(skip_all)]
    async fn get_archived_task_records(
        params: &DriftRequest,
        begin: DateTime<Utc>,
        end: DateTime<Utc>,
        minutes: i32,
        storage_settings: &ObjectStorageSettings,
        entity_id: &i32,
    ) -> Result<BinnedMetrics, SqlError> {
        debug!(
            "Getting archived agent task metrics for params: {:?}",
            params
        );
        let path = format!("{}/{}", params.uid, RecordType::AgentTask);
        let bin = minutes as f64 / params.max_data_points as f64;
        let archived_df = ParquetDataFrame::new(storage_settings, &RecordType::AgentTask)?
            .get_binned_metrics(&path, &bin, &begin, &end, entity_id)
            .await
            .inspect_err(|e| {
                error!("Failed to get archived agent task metrics: {:?}", e);
            })?;

        Ok(BinnedMetricsExtractor::dataframe_to_binned_metrics(archived_df).await?)
    }

    #[instrument(skip_all)]
    async fn get_archived_workflow_records(
        params: &DriftRequest,
        begin: DateTime<Utc>,
        end: DateTime<Utc>,
        minutes: i32,
        storage_settings: &ObjectStorageSettings,
        entity_id: &i32,
    ) -> Result<BinnedMetrics, SqlError> {
        debug!(
            "Getting archived agent workflow metrics for params: {:?}",
            params
        );
        let path = format!("{}/{}", params.uid, RecordType::AgentWorkflow);
        let bin = minutes as f64 / params.max_data_points as f64;
        let archived_df = ParquetDataFrame::new(storage_settings, &RecordType::AgentWorkflow)?
            .get_binned_metrics(&path, &bin, &begin, &end, entity_id)
            .await
            .inspect_err(|e| {
                error!("Failed to get archived agent workflow metrics: {:?}", e);
            })?;

        Ok(BinnedMetricsExtractor::dataframe_to_binned_metrics(archived_df).await?)
    }

    #[instrument(skip_all)]
    async fn get_binned_agent_task_values(
        pool: &Pool<Postgres>,
        params: &DriftRequest,
        retention_period: &i32,
        storage_settings: &ObjectStorageSettings,
        entity_id: &i32,
    ) -> Result<BinnedMetrics, SqlError> {
        debug!("Getting binned agent task drift records for {:?}", params);

        if !params.has_custom_interval() {
            let (start_dt, end_dt) = params.time_interval.to_begin_end_times()?;
            return Self::get_binned_task_records(pool, params, start_dt, end_dt, entity_id).await;
        }

        let interval = params.clone().to_custom_interval().unwrap();
        let timestamps = split_custom_interval(interval.begin, interval.end, retention_period)?;
        let mut custom_metric_map = BinnedMetrics::default();

        if let Some((active_begin, active_end)) = timestamps.active_range {
            let current_results =
                Self::get_binned_task_records(pool, params, active_begin, active_end, entity_id)
                    .await?;
            Self::merge_feature_results(current_results, &mut custom_metric_map)?;
        }

        if let Some((archive_begin, archive_end)) = timestamps.archived_range
            && let Some(archived_minutes) = timestamps.archived_minutes
        {
            let archived_results = Self::get_archived_task_records(
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

        Ok(custom_metric_map)
    }

    #[instrument(skip_all)]
    async fn get_binned_agent_workflow_values(
        pool: &Pool<Postgres>,
        params: &DriftRequest,
        retention_period: &i32,
        storage_settings: &ObjectStorageSettings,
        entity_id: &i32,
    ) -> Result<BinnedMetrics, SqlError> {
        debug!(
            "Getting binned agent workflow drift records for {:?}",
            params
        );

        if !params.has_custom_interval() {
            let (start_dt, end_dt) = params.time_interval.to_begin_end_times()?;
            return Self::get_binned_workflow_records(pool, params, start_dt, end_dt, entity_id)
                .await;
        }

        let interval = params.clone().to_custom_interval().unwrap();
        let timestamps = split_custom_interval(interval.begin, interval.end, retention_period)?;
        let mut custom_metric_map = BinnedMetrics::default();

        if let Some((active_begin, active_end)) = timestamps.active_range {
            let current_results = Self::get_binned_workflow_records(
                pool,
                params,
                active_begin,
                active_end,
                entity_id,
            )
            .await?;
            Self::merge_feature_results(current_results, &mut custom_metric_map)?;
        }

        if let Some((archive_begin, archive_end)) = timestamps.archived_range
            && let Some(archived_minutes) = timestamps.archived_minutes
        {
            let archived_results = Self::get_archived_workflow_records(
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

        debug!(
            "Custom metric map length: {:?}",
            custom_metric_map.metrics.len()
        );

        Ok(custom_metric_map)
    }

    async fn get_pending_agent_eval_record(
        pool: &Pool<Postgres>,
        max_retries: i32,
    ) -> Result<Option<EvalRecord>, SqlError> {
        let query = Queries::GetPendingAgentEvalTask.get_query();
        let result: Option<EvalRecord> = sqlx::query_as(query)
            .bind(max_retries)
            .fetch_optional(pool)
            .await
            .map_err(SqlError::SqlxError)?;

        debug!("Fetched pending agent eval record: {:?}", result);

        Ok(result)
    }

    #[instrument(skip_all)]
    async fn update_agent_eval_record_status(
        pool: &Pool<Postgres>,
        record: &EvalRecord,
        status: Status,
        workflow_duration: &i64,
    ) -> Result<(), SqlError> {
        let query = Queries::UpdateAgentEvalTask.get_query();
        sqlx::query(query)
            .bind(status.as_str())
            .bind(workflow_duration)
            .bind(&record.uid)
            .execute(pool)
            .await
            .inspect_err(|e| {
                error!("Failed to update agent eval record status: {:?}", e);
            })?;

        Ok(())
    }

    #[instrument(skip_all)]
    async fn reschedule_agent_eval_record(
        pool: &Pool<Postgres>,
        uid: &str,
        delay: Duration,
    ) -> Result<(), SqlError> {
        let scheduled_at = Utc::now() + delay;

        let query = Queries::RescheduleEvalRecord.get_query();
        sqlx::query(query)
            .bind(scheduled_at)
            .bind(uid)
            .execute(pool)
            .await
            .inspect_err(|e| {
                error!("Failed to reschedule agent eval record: {:?}", e);
            })?;

        Ok(())
    }
}
