// Module for polling GenAI drift records that are "pending" and need to be processed
use crate::error::DriftError;
use chrono::Duration;
use scouter_dataframe::parquet::tracing::service::get_trace_span_service;
use scouter_evaluate::evaluate::AgentEvaluator;
use scouter_sql::PostgresClient;
use scouter_sql::sql::traits::{AgentDriftSqlLogic, ProfileSqlLogic};
use scouter_types::agent::{AgentEvalProfile, EvalSet};
use scouter_types::sql::TraceSpan;
use scouter_types::{
    EvalRecord, SCOUTER_EVAL_PROFILE_UID, SCOUTER_EVAL_RECORD_UID, Status, TraceId,
};
use sqlx::{Pool, Postgres};
use std::sync::Arc;
use tokio::time::sleep;
use tracing::{debug, error, instrument};

#[instrument(skip_all)]
async fn get_trace_spans_by_id(trace_id: &TraceId) -> Result<Arc<Vec<TraceSpan>>, DriftError> {
    let span_service = get_trace_span_service().ok_or_else(|| {
        DriftError::AgentEvaluatorError("TraceSpanService not initialized".to_string())
    })?;

    let spans = span_service
        .query_service
        .get_trace_spans(
            Some(trace_id.as_bytes().as_slice()),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .await
        .map_err(|e| DriftError::AgentEvaluatorError(format!("Error fetching spans: {}", e)))?;

    Ok(Arc::new(spans))
}

fn span_has_attribute(span: &TraceSpan, key: &str, expected: &str) -> bool {
    span.attributes
        .iter()
        .any(|attr| attr.key == key && attr.value.as_str() == Some(expected))
}

fn validate_trace_anchor(
    task: &EvalRecord,
    profile: &AgentEvalProfile,
    spans: &[TraceSpan],
) -> Result<(), DriftError> {
    let Some(span_id) = &task.span_id else {
        return Err(DriftError::AgentEvaluatorError(format!(
            "Trace-attached eval record {} is missing span_id",
            task.uid
        )));
    };

    let expected_span_id = span_id.to_hex();
    let Some(span) = spans.iter().find(|span| span.span_id == expected_span_id) else {
        return Err(DriftError::AgentEvaluatorError(format!(
            "Trace was found for eval record {} but span {} was not present",
            task.uid, expected_span_id
        )));
    };

    if !span_has_attribute(span, SCOUTER_EVAL_RECORD_UID, &task.uid)
        || !span_has_attribute(span, SCOUTER_EVAL_PROFILE_UID, &profile.config.uid)
    {
        return Err(DriftError::AgentEvaluatorError(format!(
            "Trace was found for eval record {} but failed eval association validation",
            task.uid
        )));
    }

    Ok(())
}

/// Poller struct for processing GenAI drift records
/// A few different things going on here:
/// 1. Poll the database for "pending" GenAI drift records
/// 2. For each record, check if trace spans are needed and available
/// 3. If spans are needed but not available, reschedule the record for later processing
/// 4. If spans are available or not needed, process the record using AgentEvaluator
/// 5. Update the record status to "processed" or "failed" based on the outcome
pub struct AgentPoller {
    db_pool: Pool<Postgres>,
    max_retries: i32,
}

impl AgentPoller {
    pub fn new(
        db_pool: &Pool<Postgres>,
        max_retries: i32,
        _trace_wait_timeout: Duration,
        _trace_backoff: Duration,
        _trace_reschedule_delay: Duration,
    ) -> Self {
        AgentPoller {
            db_pool: db_pool.clone(),
            max_retries,
        }
    }

    #[instrument(skip_all)]
    pub async fn process_event_record(
        &mut self,
        record: &EvalRecord,
        profile: &AgentEvalProfile,
        spans: Arc<Vec<TraceSpan>>,
    ) -> Result<EvalSet, DriftError> {
        debug!("Processing workflow");

        // create arc mutex for profile
        let profile = Arc::new(profile.clone());

        match AgentEvaluator::process_event_record(record, profile, spans).await {
            Ok(result_set) => {
                // insert task results first
                PostgresClient::insert_eval_task_results_batch(
                    &self.db_pool,
                    &result_set.records,
                    &record.entity_id,
                )
                .await
                .inspect_err(|e| {
                    error!("Failed to insert LLM task results: {:?}", e);
                })?;

                // insert workflow record
                PostgresClient::insert_agent_eval_workflow_record(
                    &self.db_pool,
                    &result_set.inner,
                    &record.entity_id,
                )
                .await
                .inspect_err(|e| {
                    error!("Failed to insert Agent workflow record: {:?}", e);
                })?;

                return Ok(result_set);
            }
            Err(e) => {
                error!("Failed to process drift record: {:?}", e);
                return Err(DriftError::AgentEvaluatorError(e.to_string()));
            }
        };
    }

    #[instrument(skip_all)]
    pub async fn do_poll(&mut self) -> Result<bool, DriftError> {
        let task =
            PostgresClient::get_pending_agent_eval_record(&self.db_pool, self.max_retries).await?;

        let Some(task) = task else {
            return Ok(false);
        };

        debug!("Processing genai drift record for profile: {}", task.uid);

        let mut genai_profile = if let Some(profile) =
            PostgresClient::get_drift_profile(&self.db_pool, &task.entity_id).await?
        {
            let genai_profile: AgentEvalProfile =
                serde_json::from_value(profile).inspect_err(|e| {
                    error!("Failed to deserialize agent evaluation profile: {:?}", e);
                })?;
            genai_profile
        } else {
            error!("No agent evaluation profile found for {}", task.uid);
            return Ok(false);
        };

        let mut retry_count = 0;
        if let Some(workflow) = &mut genai_profile.workflow {
            workflow.reset_agents().await.inspect_err(|e| {
                error!("Failed to reset agents: {:?}", e);
            })?;
        }

        let spans = if genai_profile.has_trace_assertions() {
            match (&task.trace_id, &task.span_id) {
                (Some(trace_id), Some(_span_id)) => match get_trace_spans_by_id(trace_id).await {
                    Ok(spans) if !spans.is_empty() => {
                        if let Err(e) = validate_trace_anchor(&task, &genai_profile, &spans) {
                            error!(
                                trace_id = %trace_id.to_hex(),
                                task_uid = %task.uid,
                                error = %e,
                                "Trace anchor failed eval association validation"
                            );
                            PostgresClient::update_agent_eval_record_status(
                                &self.db_pool,
                                &task,
                                Status::Failed,
                                &0,
                            )
                            .await?;
                            return Err(e);
                        }
                        spans
                    }
                    Ok(_) | Err(_) => {
                        error!(
                            trace_id = %trace_id.to_hex(),
                            "Direct fetch returned no spans for task {} despite inbox flip",
                            task.uid
                        );
                        PostgresClient::update_agent_eval_record_status(
                            &self.db_pool,
                            &task,
                            Status::Failed,
                            &0,
                        )
                        .await?;
                        return Err(DriftError::AgentEvaluatorError(format!(
                            "Trace spans not available for task: {}",
                            task.uid
                        )));
                    }
                },
                _ => {
                    error!(
                        "Trace assertions require records created by span.attach_eval with both trace_id and span_id; task {} is missing a complete trace anchor",
                        task.uid
                    );
                    PostgresClient::update_agent_eval_record_status(
                        &self.db_pool,
                        &task,
                        Status::Failed,
                        &0,
                    )
                    .await?;
                    return Err(DriftError::AgentEvaluatorError(format!(
                        "Trace-attached eval record {} is missing trace_id or span_id",
                        task.uid
                    )));
                }
            }
        } else {
            Arc::new(vec![])
        };

        loop {
            match self
                .process_event_record(&task, &genai_profile, spans.clone())
                .await
            {
                Ok(result_set) => {
                    PostgresClient::update_agent_eval_record_status(
                        &self.db_pool,
                        &task,
                        Status::Processed,
                        &result_set.inner.duration_ms,
                    )
                    .await?;
                    break;
                }
                Err(e) => {
                    error!(
                        "Failed to process drift record (attempt {}): {:?}",
                        retry_count + 1,
                        e
                    );

                    retry_count += 1;
                    if retry_count >= self.max_retries {
                        // Update the record status to error
                        PostgresClient::update_agent_eval_record_status(
                            &self.db_pool,
                            &task,
                            Status::Failed,
                            &0,
                        )
                        .await?;
                        return Err(DriftError::AgentEvaluatorError(e.to_string()));
                    } else {
                        // Exponential backoff before retrying
                        let val = 100 * 2_i64.pow(retry_count as u32);
                        sleep(Duration::milliseconds(val).to_std()?).await;
                    }
                }
            }
        }

        Ok(true)
    }

    #[instrument(skip_all)]
    pub async fn poll_for_tasks(&mut self) -> Result<(), DriftError> {
        let result = self.do_poll().await;

        // silent error handling
        match result {
            Ok(true) => {
                debug!("Successfully processed drift record");
                Ok(())
            }
            Ok(false) => {
                sleep(Duration::seconds(1).to_std()?).await;
                Ok(())
            }
            Err(e) => {
                error!("Error processing drift record: {:?}", e);
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use scouter_types::{Attribute, SpanId};
    use serde_json::json;

    #[tokio::test]
    async fn get_trace_spans_by_id_fails_when_service_not_initialized() {
        let trace_id = TraceId::from_bytes([0u8; 16]);
        let result = get_trace_spans_by_id(&trace_id).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("TraceSpanService not initialized"),
            "unexpected error: {}",
            err_msg
        );
    }

    #[test]
    fn eval_record_trace_id_none_by_default() {
        let record = EvalRecord {
            trace_id: None,
            ..Default::default()
        };
        assert!(record.trace_id.is_none());
    }

    #[test]
    fn eval_record_trace_id_some_when_set() {
        let trace_id = TraceId::from_bytes([1u8; 16]);
        let record = EvalRecord {
            trace_id: Some(trace_id),
            ..Default::default()
        };
        assert!(record.trace_id.is_some());
        assert_eq!(record.trace_id.unwrap().to_hex(), trace_id.to_hex());
    }

    fn make_span(span_id: &SpanId, record_uid: &str, profile_uid: &str) -> TraceSpan {
        let now = Utc::now();
        TraceSpan {
            trace_id: TraceId::from_bytes([1; 16]).to_hex(),
            span_id: span_id.to_hex(),
            parent_span_id: None,
            span_name: "agent.run".to_string(),
            span_kind: None,
            start_time: now,
            end_time: now,
            duration_ms: 0,
            status_code: 0,
            status_message: None,
            attributes: vec![
                Attribute {
                    key: SCOUTER_EVAL_RECORD_UID.to_string(),
                    value: json!(record_uid),
                },
                Attribute {
                    key: SCOUTER_EVAL_PROFILE_UID.to_string(),
                    value: json!(profile_uid),
                },
            ],
            events: Vec::new(),
            links: Vec::new(),
            depth: 0,
            path: Vec::new(),
            root_span_id: span_id.to_hex(),
            service_name: "svc".to_string(),
            service_namespace: None,
            service_version: None,
            service_instance_id: None,
            span_order: 0,
            input: None,
            output: None,
        }
    }

    #[test]
    fn validate_trace_anchor_rejects_profile_mismatch() {
        let span_id = SpanId::from_bytes([9; 8]);
        let mut profile = AgentEvalProfile::default();
        profile.config.uid = "profile-a".to_string();
        let task = EvalRecord {
            uid: "record-a".to_string(),
            span_id: Some(span_id.clone()),
            ..Default::default()
        };
        let spans = vec![make_span(&span_id, "record-a", "profile-b")];

        let err = validate_trace_anchor(&task, &profile, &spans).unwrap_err();
        assert!(
            err.to_string()
                .contains("failed eval association validation")
        );
    }

    #[test]
    fn validate_trace_anchor_accepts_exact_record_and_profile_match() {
        let span_id = SpanId::from_bytes([8; 8]);
        let mut profile = AgentEvalProfile::default();
        profile.config.uid = "profile-a".to_string();
        let task = EvalRecord {
            uid: "record-a".to_string(),
            span_id: Some(span_id.clone()),
            ..Default::default()
        };
        let spans = vec![make_span(&span_id, "record-a", "profile-a")];

        validate_trace_anchor(&task, &profile, &spans).unwrap();
    }
}
