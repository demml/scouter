use crate::common::{NAME, SPACE, VERSION, setup_test};
use chrono::Utc;
use potato_head::create_uuid7;
use scouter_mocks::generate_trace_with_spans;
use scouter_sql::PostgresClient;
use scouter_sql::sql::traits::{AgentDriftSqlLogic, EntitySqlLogic};
use scouter_types::agent::{
    AgentAlertConfig, AgentEvalConfig, AgentEvalProfile, AssertionTask, ComparisonOperator,
    EvaluationTaskType, EvaluationTasks, TraceAssertion, TraceAssertionTask,
};
use scouter_types::{
    BoxedEvalRecord, DriftType, EvalRecord, MessageRecord, ServerRecord, ServerRecords, SpanId,
    Status, TraceId,
};
use serde_json::{Value, json};
use sqlx::{Pool, Postgres};
use std::time::Duration;

async fn trace_profile() -> AgentEvalProfile {
    let task = TraceAssertionTask {
        id: "trace_span_count".to_string(),
        assertion: TraceAssertion::TraceSpanCount {},
        operator: ComparisonOperator::GreaterThanOrEqual,
        expected_value: json!(1),
        description: None,
        depends_on: Vec::new(),
        task_type: EvaluationTaskType::TraceAssertion,
        result: None,
        condition: false,
    };
    let config =
        AgentEvalConfig::new(SPACE, NAME, VERSION, 1.0, AgentAlertConfig::default(), None).unwrap();

    AgentEvalProfile::new(config, EvaluationTasks::new().add_task(task).build())
        .await
        .unwrap()
}

async fn content_profile() -> AgentEvalProfile {
    let task = AssertionTask {
        id: "input_exists".to_string(),
        context_path: Some("input".to_string()),
        item_context_path: None,
        operator: ComparisonOperator::Equals,
        expected_value: json!("hello"),
        description: None,
        depends_on: Vec::new(),
        task_type: EvaluationTaskType::Assertion,
        result: None,
        condition: false,
    };
    let config = AgentEvalConfig::new(
        SPACE,
        "content-only",
        VERSION,
        1.0,
        AgentAlertConfig::default(),
        None,
    )
    .unwrap();

    AgentEvalProfile::new(config, EvaluationTasks::new().add_task(task).build())
        .await
        .unwrap()
}

fn eval_message(
    uid: &str,
    trace_id: Option<TraceId>,
    span_id: Option<SpanId>,
    record_uid: &str,
) -> MessageRecord {
    let record = EvalRecord {
        created_at: Utc::now(),
        entity_uid: uid.to_string(),
        context: json!({"input": "hello"}),
        uid: record_uid.to_string(),
        trace_id,
        span_id,
        ..Default::default()
    };

    MessageRecord::ServerRecords(ServerRecords::new(vec![ServerRecord::AgentEval(
        BoxedEvalRecord::new(record),
    )]))
}

async fn insert_message(helper: &crate::common::TestHelper, message: MessageRecord) {
    let client = helper.create_grpc_client().await;
    let response = client
        .insert_message(serde_json::to_vec(&message).unwrap())
        .await;
    assert!(response.is_ok());
}

async fn wait_for_status(pool: &Pool<Postgres>, uid: &str, status: &str) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        match sqlx::query_as::<_, (String, Value)>(
            "SELECT status, context FROM scouter.agent_eval_record WHERE uid = $1",
        )
        .bind(uid)
        .fetch_optional(pool)
        .await
        .unwrap()
        {
            Some((current_status, context)) if current_status == status => return context,
            _ => {}
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for {uid}={status}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn wait_for_event(pool: &Pool<Postgres>, trace_id: TraceId) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if PostgresClient::trace_commit_event_exists(pool, &trace_id)
            .await
            .unwrap()
        {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for inbox event"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn insert_awaiting_record(pool: &Pool<Postgres>, uid: &str, trace_id: TraceId) {
    let (_entity_uid, entity_id) = PostgresClient::create_entity(
        pool,
        SPACE,
        &format!("entity-{uid}"),
        VERSION,
        DriftType::Agent.to_string(),
    )
    .await
    .unwrap();
    let record = EvalRecord {
        created_at: Utc::now(),
        uid: uid.to_string(),
        context: json!({"input": "hello"}),
        trace_id: Some(trace_id),
        span_id: Some(SpanId::from_bytes([0x11; 8])),
        ..Default::default()
    };
    PostgresClient::insert_agent_eval_record(
        pool,
        BoxedEvalRecord::new(record),
        &entity_id,
        Status::AwaitingTrace,
    )
    .await
    .unwrap();
}

async fn processed_event_count(pool: &Pool<Postgres>, trace_id: TraceId) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM scouter.trace_commit_event WHERE trace_id = $1 AND processed_at IS NOT NULL",
    )
    .bind(trace_id.as_bytes().to_vec())
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn test_agent_trace_inbox_end_to_end_paths() {
    let helper = setup_test().await;
    let trace_profile_uid = helper
        .register_drift_profile(trace_profile().await.create_profile_request().unwrap())
        .await;
    let content_profile_uid = helper
        .register_drift_profile(content_profile().await.create_profile_request().unwrap())
        .await;

    // 1. Forward race: eval awaits trace, Delta commit emits inbox event, worker flips pending.
    let (trace_a, spans_a, _) = generate_trace_with_spans(2, 0);
    let forward_uid = create_uuid7();
    insert_message(
        &helper,
        eval_message(
            &trace_profile_uid,
            Some(trace_a.trace_id),
            Some(spans_a[0].span_id.clone()),
            &forward_uid,
        ),
    )
    .await;
    wait_for_status(&helper.pool, &forward_uid, "awaiting_trace").await;
    helper
        .trace_service
        .write_spans_direct(spans_a)
        .await
        .unwrap();
    wait_for_status(&helper.pool, &forward_uid, "pending").await;
    assert_eq!(
        processed_event_count(&helper.pool, trace_a.trace_id).await,
        1
    );

    // 2. Reverse race: committed trace is cached in inbox, so eval inserts as pending immediately.
    let (trace_b, spans_b, _) = generate_trace_with_spans(2, 0);
    helper
        .trace_service
        .write_spans_direct(spans_b.clone())
        .await
        .unwrap();
    wait_for_event(&helper.pool, trace_b.trace_id).await;
    let reverse_uid = create_uuid7();
    insert_message(
        &helper,
        eval_message(
            &trace_profile_uid,
            Some(trace_b.trace_id),
            Some(spans_b[0].span_id.clone()),
            &reverse_uid,
        ),
    )
    .await;
    wait_for_status(&helper.pool, &reverse_uid, "pending").await;

    let negative_uid = create_uuid7();
    insert_message(
        &helper,
        eval_message(
            &trace_profile_uid,
            Some(TraceId::from_bytes([0x44; 16])),
            Some(SpanId::from_bytes([0x44; 8])),
            &negative_uid,
        ),
    )
    .await;
    wait_for_status(&helper.pool, &negative_uid, "awaiting_trace").await;

    // 3. Crash recovery simulation: durable event + awaiting eval drains to pending.
    let crash_trace = TraceId::from_bytes([0x55; 16]);
    let crash_uid = create_uuid7();
    PostgresClient::insert_trace_commit_events(&helper.pool, &[crash_trace])
        .await
        .unwrap();
    insert_awaiting_record(&helper.pool, &crash_uid, crash_trace).await;
    scouter_drift::genai::test_helpers::drain_once(&helper.pool)
        .await
        .unwrap();
    wait_for_status(&helper.pool, &crash_uid, "pending").await;
    assert_eq!(processed_event_count(&helper.pool, crash_trace).await, 1);

    // 4. Content-only profile does not need a trace.
    let content_uid = create_uuid7();
    insert_message(
        &helper,
        eval_message(&content_profile_uid, None, None, &content_uid),
    )
    .await;
    wait_for_status(&helper.pool, &content_uid, "pending").await;

    // 5. Trace-needing profile without trace fails terminally.
    let missing_trace_uid = create_uuid7();
    insert_message(
        &helper,
        eval_message(&trace_profile_uid, None, None, &missing_trace_uid),
    )
    .await;
    let context = wait_for_status(&helper.pool, &missing_trace_uid, "failed").await;
    assert_eq!(context["error"], "EvalRequiresTrace");

    // 6. Timeout sweep fails stale awaiting_trace rows.
    let timeout_uid = create_uuid7();
    insert_awaiting_record(&helper.pool, &timeout_uid, TraceId::from_bytes([0x66; 16])).await;
    sqlx::query(
        "UPDATE scouter.agent_eval_record SET created_at = now() - interval '6 minutes' WHERE uid = $1",
    )
    .bind(&timeout_uid)
    .execute(&helper.pool)
    .await
    .unwrap();
    scouter_drift::genai::test_helpers::run_sweeps(&helper.pool).await;
    let context = wait_for_status(&helper.pool, &timeout_uid, "failed").await;
    assert_eq!(context["error"], "TraceArrivalTimeout");

    // 7. Processed inbox prune keeps recent and unprocessed rows.
    sqlx::query(
        "INSERT INTO scouter.trace_commit_event (trace_id, processed_at) VALUES ($1, now() - interval '25 hours')",
    )
    .bind(TraceId::from_bytes([0x77; 16]).as_bytes().to_vec())
    .execute(&helper.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO scouter.trace_commit_event (trace_id, processed_at) VALUES ($1, now() - interval '1 hour')",
    )
    .bind(TraceId::from_bytes([0x78; 16]).as_bytes().to_vec())
    .execute(&helper.pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO scouter.trace_commit_event (trace_id) VALUES ($1)")
        .bind(TraceId::from_bytes([0x79; 16]).as_bytes().to_vec())
        .execute(&helper.pool)
        .await
        .unwrap();
    scouter_drift::genai::test_helpers::run_sweeps(&helper.pool).await;
    assert!(
        !PostgresClient::trace_commit_event_exists(&helper.pool, &TraceId::from_bytes([0x77; 16]))
            .await
            .unwrap()
    );
    assert!(
        PostgresClient::trace_commit_event_exists(&helper.pool, &TraceId::from_bytes([0x78; 16]))
            .await
            .unwrap()
    );
    assert!(
        PostgresClient::trace_commit_event_exists(&helper.pool, &TraceId::from_bytes([0x79; 16]))
            .await
            .unwrap()
    );

    // 8. Poll SQL hydrates span_id into EvalRecord for the direct trace-id arm.
    sqlx::query(
        "UPDATE scouter.agent_eval_record SET status = 'processed' WHERE status = 'pending'",
    )
    .execute(&helper.pool)
    .await
    .unwrap();
    let poll_uid = create_uuid7();
    let poll_trace = TraceId::from_bytes([0x88; 16]);
    let poll_span = SpanId::from_bytes([0x88; 8]);
    let (_entity_uid, entity_id) = PostgresClient::create_entity(
        &helper.pool,
        SPACE,
        "poll-span-roundtrip",
        VERSION,
        DriftType::Agent.to_string(),
    )
    .await
    .unwrap();
    let record = EvalRecord {
        created_at: Utc::now(),
        uid: poll_uid,
        context: json!({"input": "hello"}),
        trace_id: Some(poll_trace),
        span_id: Some(poll_span.clone()),
        ..Default::default()
    };
    PostgresClient::insert_agent_eval_record(
        &helper.pool,
        BoxedEvalRecord::new(record),
        &entity_id,
        Status::Pending,
    )
    .await
    .unwrap();
    let pending = PostgresClient::get_pending_agent_eval_record(&helper.pool, 3)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(pending.span_id, Some(poll_span));

    // 9. Lost event acceptance: no inbox row means stale awaiting_trace times out.
    let lost_uid = create_uuid7();
    insert_awaiting_record(&helper.pool, &lost_uid, TraceId::from_bytes([0x99; 16])).await;
    sqlx::query(
        "UPDATE scouter.agent_eval_record SET created_at = now() - interval '6 minutes' WHERE uid = $1",
    )
    .bind(&lost_uid)
    .execute(&helper.pool)
    .await
    .unwrap();
    scouter_drift::genai::test_helpers::run_sweeps(&helper.pool).await;
    let context = wait_for_status(&helper.pool, &lost_uid, "failed").await;
    assert_eq!(context["error"], "TraceArrivalTimeout");

    // 10. Multi-pod claim concurrency: two drains process one shared inbox without double-work.
    let mut trace_ids = Vec::new();
    for offset in 0u8..100 {
        let byte = 0xA0u8.wrapping_add(offset);
        let trace_id = TraceId::from_bytes([byte; 16]);
        trace_ids.push(trace_id);
        insert_awaiting_record(&helper.pool, &format!("concurrent-{byte}"), trace_id).await;
    }
    PostgresClient::insert_trace_commit_events(&helper.pool, &trace_ids)
        .await
        .unwrap();
    let (left, right) = tokio::join!(
        scouter_drift::genai::test_helpers::drain_once(&helper.pool),
        scouter_drift::genai::test_helpers::drain_once(&helper.pool)
    );
    left.unwrap();
    right.unwrap();
    let pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM scouter.agent_eval_record WHERE uid LIKE 'concurrent-%' AND status = 'pending'",
    )
    .fetch_one(&helper.pool)
    .await
    .unwrap();
    assert_eq!(pending, 100);
    let processed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM scouter.trace_commit_event WHERE trace_id = ANY($1) AND processed_at IS NOT NULL",
    )
    .bind(
        trace_ids
            .iter()
            .map(|trace_id| trace_id.as_bytes().to_vec())
            .collect::<Vec<_>>(),
    )
    .fetch_one(&helper.pool)
    .await
    .unwrap();
    assert_eq!(processed, 100);
}
