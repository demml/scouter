use crate::common::{NAME, SPACE, VERSION, setup_test};
use chrono::Utc;
use potato_head::create_uuid7;
use scouter_drift::genai::AgentPoller;
use scouter_mocks::generate_trace_with_spans;
use scouter_sql::PostgresClient;
use scouter_sql::sql::traits::{AgentDriftSqlLogic, EntitySqlLogic};
use scouter_types::agent::{
    AgentAlertConfig, AgentEvalConfig, AgentEvalProfile, AssertionTask, ComparisonOperator,
    EvaluationTaskType, EvaluationTasks, TraceAssertion, TraceAssertionTask,
};
use scouter_types::{
    Attribute, BoxedEvalRecord, DriftType, EvalRecord, MessageRecord, SCOUTER_EVAL_PROFILE_UID,
    SCOUTER_EVAL_RECORD_UID, ServerRecord, ServerRecords, SpanId, Status, TraceCommitAnchor,
    TraceId,
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

#[tokio::test]
async fn test_direct_pending_trace_record_poller_processes() {
    let helper = setup_test().await;
    let trace_profile_uid = helper
        .register_drift_profile(trace_profile().await.create_profile_request().unwrap())
        .await;
    let entity_id = PostgresClient::get_entity_id_from_uid(&helper.pool, &trace_profile_uid)
        .await
        .unwrap();

    let (trace, mut spans, _) = generate_trace_with_spans(2, 0);
    let span_id = spans[0].span_id.clone();
    let record_uid = create_uuid7();
    spans[0].attributes.extend([
        Attribute {
            key: SCOUTER_EVAL_RECORD_UID.to_string(),
            value: json!(record_uid.clone()),
        },
        Attribute {
            key: SCOUTER_EVAL_PROFILE_UID.to_string(),
            value: json!(trace_profile_uid.clone()),
        },
    ]);
    helper
        .trace_service
        .write_spans_direct(spans)
        .await
        .unwrap();

    let record = EvalRecord {
        created_at: Utc::now(),
        uid: record_uid.clone(),
        entity_id,
        context: json!({"input": "hello"}),
        trace_id: Some(trace.trace_id),
        span_id: Some(span_id),
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

    let mut poller = AgentPoller::new(
        &helper.pool,
        3,
        chrono::Duration::seconds(1),
        chrono::Duration::milliseconds(10),
        chrono::Duration::seconds(1),
    );
    assert!(poller.do_poll().await.unwrap());

    let status: String =
        sqlx::query_scalar("SELECT status FROM scouter.agent_eval_record WHERE uid = $1")
            .bind(&record_uid)
            .fetch_one(&helper.pool)
            .await
            .unwrap();
    assert_eq!(status, "processed");

    let task_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM scouter.agent_eval_task WHERE record_uid = $1")
            .bind(&record_uid)
            .fetch_one(&helper.pool)
            .await
            .unwrap();
    assert_eq!(task_count, 1);

    let workflow_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM scouter.agent_eval_workflow WHERE record_uid = $1",
    )
    .bind(&record_uid)
    .fetch_one(&helper.pool)
    .await
    .unwrap();
    assert_eq!(workflow_count, 1);
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

fn anchor(
    trace_id: TraceId,
    span_id: SpanId,
    record_uid: &str,
    profile_uid: &str,
) -> TraceCommitAnchor {
    TraceCommitAnchor {
        trace_id,
        span_id,
        record_uid: record_uid.to_string(),
        profile_uid: profile_uid.to_string(),
    }
}

fn stamp_anchor(span: &mut scouter_types::TraceSpanRecord, record_uid: &str, profile_uid: &str) {
    span.attributes.extend([
        Attribute {
            key: SCOUTER_EVAL_RECORD_UID.to_string(),
            value: json!(record_uid),
        },
        Attribute {
            key: SCOUTER_EVAL_PROFILE_UID.to_string(),
            value: json!(profile_uid),
        },
    ]);
}

async fn wait_for_event(pool: &Pool<Postgres>, anchor: &TraceCommitAnchor) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if PostgresClient::trace_commit_event_exists(pool, anchor)
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

async fn processed_event_count(pool: &Pool<Postgres>, record_uid: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM scouter.trace_commit_event WHERE record_uid = $1 AND status = 'processed'",
    )
    .bind(record_uid)
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
    let (trace_a, mut spans_a, _) = generate_trace_with_spans(2, 0);
    let forward_uid = create_uuid7();
    stamp_anchor(&mut spans_a[0], &forward_uid, &trace_profile_uid);
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
    assert_eq!(processed_event_count(&helper.pool, &forward_uid).await, 1);

    // 2. Reverse race: committed trace is cached in inbox, so eval inserts as pending immediately.
    let (trace_b, mut spans_b, _) = generate_trace_with_spans(2, 0);
    let reverse_uid = create_uuid7();
    stamp_anchor(&mut spans_b[0], &reverse_uid, &trace_profile_uid);
    let reverse_anchor = anchor(
        trace_b.trace_id,
        spans_b[0].span_id.clone(),
        &reverse_uid,
        &trace_profile_uid,
    );
    helper
        .trace_service
        .write_spans_direct(spans_b.clone())
        .await
        .unwrap();
    wait_for_event(&helper.pool, &reverse_anchor).await;
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
    PostgresClient::insert_trace_commit_events(
        &helper.pool,
        &[anchor(
            crash_trace,
            SpanId::from_bytes([0x11; 8]),
            &crash_uid,
            &trace_profile_uid,
        )],
    )
    .await
    .unwrap();
    insert_awaiting_record(&helper.pool, &crash_uid, crash_trace).await;
    scouter_drift::genai::test_helpers::drain_once(&helper.pool)
        .await
        .unwrap();
    wait_for_status(&helper.pool, &crash_uid, "pending").await;
    assert_eq!(processed_event_count(&helper.pool, &crash_uid).await, 1);

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

    // 6. Trace-needing profile with trace_id but no span_id fails terminally.
    let missing_span_uid = create_uuid7();
    insert_message(
        &helper,
        eval_message(
            &trace_profile_uid,
            Some(TraceId::from_bytes([0x45; 16])),
            None,
            &missing_span_uid,
        ),
    )
    .await;
    let context = wait_for_status(&helper.pool, &missing_span_uid, "failed").await;
    assert_eq!(context["error"], "EvalRequiresAnchorSpan");

    // 7. Timeout sweep fails stale awaiting_trace rows.
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

    // 8. Processed inbox prune keeps recent and unprocessed rows.
    let prune_old = anchor(
        TraceId::from_bytes([0x77; 16]),
        SpanId::from_bytes([0x77; 8]),
        "prune-old",
        "profile",
    );
    let prune_fresh = anchor(
        TraceId::from_bytes([0x78; 16]),
        SpanId::from_bytes([0x78; 8]),
        "prune-fresh",
        "profile",
    );
    let prune_pending = anchor(
        TraceId::from_bytes([0x79; 16]),
        SpanId::from_bytes([0x79; 8]),
        "prune-pending",
        "profile",
    );
    sqlx::query(
        "INSERT INTO scouter.trace_commit_event (trace_id, span_id, record_uid, profile_uid, status, processed_at) VALUES ($1, $2, $3, $4, 'processed', now() - interval '25 hours')",
    )
    .bind(prune_old.trace_id.as_bytes().to_vec())
    .bind(prune_old.span_id.as_bytes().to_vec())
    .bind(&prune_old.record_uid)
    .bind(&prune_old.profile_uid)
    .execute(&helper.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO scouter.trace_commit_event (trace_id, span_id, record_uid, profile_uid, status, processed_at) VALUES ($1, $2, $3, $4, 'processed', now() - interval '1 hour')",
    )
    .bind(prune_fresh.trace_id.as_bytes().to_vec())
    .bind(prune_fresh.span_id.as_bytes().to_vec())
    .bind(&prune_fresh.record_uid)
    .bind(&prune_fresh.profile_uid)
    .execute(&helper.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO scouter.trace_commit_event (trace_id, span_id, record_uid, profile_uid) VALUES ($1, $2, $3, $4)",
    )
        .bind(prune_pending.trace_id.as_bytes().to_vec())
        .bind(prune_pending.span_id.as_bytes().to_vec())
        .bind(&prune_pending.record_uid)
        .bind(&prune_pending.profile_uid)
        .execute(&helper.pool)
        .await
        .unwrap();
    scouter_drift::genai::test_helpers::run_sweeps(&helper.pool).await;
    assert!(
        !PostgresClient::trace_commit_event_exists(&helper.pool, &prune_old)
            .await
            .unwrap()
    );
    assert!(
        PostgresClient::trace_commit_event_exists(&helper.pool, &prune_fresh)
            .await
            .unwrap()
    );
    assert!(
        PostgresClient::trace_commit_event_exists(&helper.pool, &prune_pending)
            .await
            .unwrap()
    );

    // 9. Poll SQL hydrates span_id into EvalRecord for the direct trace-id arm.
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

    // 10. Lost event acceptance: no inbox row means stale awaiting_trace times out.
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

    // 11. Multi-pod claim concurrency: two drains process one shared inbox without double-work.
    let mut anchors = Vec::new();
    let mut record_uids = Vec::new();
    for offset in 0u8..100 {
        let byte = 0xA0u8.wrapping_add(offset);
        let trace_id = TraceId::from_bytes([byte; 16]);
        let record_uid = format!("concurrent-{byte}");
        anchors.push(anchor(
            trace_id,
            SpanId::from_bytes([0x11; 8]),
            &record_uid,
            &trace_profile_uid,
        ));
        record_uids.push(record_uid.clone());
        insert_awaiting_record(&helper.pool, &record_uid, trace_id).await;
    }
    PostgresClient::insert_trace_commit_events(&helper.pool, &anchors)
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
        "SELECT count(*) FROM scouter.trace_commit_event WHERE record_uid = ANY($1::text[]) AND status = 'processed'",
    )
    .bind(record_uids)
    .fetch_one(&helper.pool)
    .await
    .unwrap();
    assert_eq!(processed, 100);
}

#[tokio::test]
async fn test_anchor_span_arriving_late_does_not_flip_eval() {
    let helper = setup_test().await;
    let trace_profile_uid = helper
        .register_drift_profile(trace_profile().await.create_profile_request().unwrap())
        .await;

    let (trace, mut spans, _) = generate_trace_with_spans(2, 0);
    let record_uid = create_uuid7();
    let anchor_span_id = spans[1].span_id.clone();
    stamp_anchor(&mut spans[1], &record_uid, &trace_profile_uid);

    insert_message(
        &helper,
        eval_message(
            &trace_profile_uid,
            Some(trace.trace_id),
            Some(anchor_span_id),
            &record_uid,
        ),
    )
    .await;
    wait_for_status(&helper.pool, &record_uid, "awaiting_trace").await;

    helper
        .trace_service
        .write_spans_direct(vec![spans[0].clone()])
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    let status: String =
        sqlx::query_scalar("SELECT status FROM scouter.agent_eval_record WHERE uid = $1")
            .bind(&record_uid)
            .fetch_one(&helper.pool)
            .await
            .unwrap();
    assert_eq!(status, "awaiting_trace");

    let inbox_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM scouter.trace_commit_event WHERE record_uid = $1")
            .bind(&record_uid)
            .fetch_one(&helper.pool)
            .await
            .unwrap();
    assert_eq!(inbox_count, 0);

    helper
        .trace_service
        .write_spans_direct(vec![spans[1].clone()])
        .await
        .unwrap();
    wait_for_status(&helper.pool, &record_uid, "pending").await;
}

#[tokio::test]
async fn test_reconciliation_recovers_lost_anchor_events() {
    let helper = setup_test().await;
    let trace_profile_uid = helper
        .register_drift_profile(trace_profile().await.create_profile_request().unwrap())
        .await;
    let entity_id = PostgresClient::get_entity_id_from_uid(&helper.pool, &trace_profile_uid)
        .await
        .unwrap();

    let (trace, mut spans, _) = generate_trace_with_spans(1, 0);
    let record_uid = create_uuid7();
    let span_id = spans[0].span_id.clone();
    stamp_anchor(&mut spans[0], &record_uid, &trace_profile_uid);
    let event_anchor = anchor(
        trace.trace_id,
        span_id.clone(),
        &record_uid,
        &trace_profile_uid,
    );

    helper
        .trace_service
        .write_spans_direct(spans)
        .await
        .unwrap();
    wait_for_event(&helper.pool, &event_anchor).await;

    sqlx::query("DELETE FROM scouter.trace_commit_event WHERE record_uid = $1")
        .bind(&record_uid)
        .execute(&helper.pool)
        .await
        .unwrap();

    let record = EvalRecord {
        created_at: Utc::now(),
        uid: record_uid.clone(),
        entity_id,
        context: json!({"input": "hello"}),
        trace_id: Some(trace.trace_id),
        span_id: Some(span_id),
        ..Default::default()
    };
    PostgresClient::insert_agent_eval_record(
        &helper.pool,
        BoxedEvalRecord::new(record),
        &entity_id,
        Status::AwaitingTrace,
    )
    .await
    .unwrap();

    let recovered = scouter_drift::genai::test_helpers::reconcile_lost_events(
        &helper.pool,
        &helper.trace_service.query_service,
    )
    .await
    .unwrap();
    assert_eq!(recovered, 1);

    scouter_drift::genai::test_helpers::drain_once(&helper.pool)
        .await
        .unwrap();
    wait_for_status(&helper.pool, &record_uid, "pending").await;
}

#[tokio::test]
async fn test_reconciliation_window_supports_long_running_anchor_spans() {
    let helper = setup_test().await;
    let trace_profile_uid = helper
        .register_drift_profile(trace_profile().await.create_profile_request().unwrap())
        .await;
    let entity_id = PostgresClient::get_entity_id_from_uid(&helper.pool, &trace_profile_uid)
        .await
        .unwrap();

    let (trace, mut spans, _) = generate_trace_with_spans(1, 0);
    let record_uid = create_uuid7();
    let span_id = spans[0].span_id.clone();
    stamp_anchor(&mut spans[0], &record_uid, &trace_profile_uid);
    let event_anchor = anchor(
        trace.trace_id,
        span_id.clone(),
        &record_uid,
        &trace_profile_uid,
    );
    let now = Utc::now();
    spans[0].start_time = now - chrono::Duration::days(2);
    spans[0].end_time = now;
    spans[0].duration_ms = (spans[0].end_time - spans[0].start_time).num_milliseconds();

    helper
        .trace_service
        .write_spans_direct(spans)
        .await
        .unwrap();
    wait_for_event(&helper.pool, &event_anchor).await;
    sqlx::query("DELETE FROM scouter.trace_commit_event WHERE record_uid = $1")
        .bind(&record_uid)
        .execute(&helper.pool)
        .await
        .unwrap();

    let record = EvalRecord {
        created_at: now,
        uid: record_uid.clone(),
        entity_id,
        context: json!({"input": "hello"}),
        trace_id: Some(trace.trace_id),
        span_id: Some(span_id),
        ..Default::default()
    };
    PostgresClient::insert_agent_eval_record(
        &helper.pool,
        BoxedEvalRecord::new(record),
        &entity_id,
        Status::AwaitingTrace,
    )
    .await
    .unwrap();

    let narrow = scouter_drift::genai::test_helpers::reconcile_lost_events_with_lookback(
        &helper.pool,
        &helper.trace_service.query_service,
        chrono::Duration::seconds(60),
    )
    .await
    .unwrap();
    assert_eq!(narrow, 0);

    let recovered = scouter_drift::genai::test_helpers::reconcile_lost_events_with_lookback(
        &helper.pool,
        &helper.trace_service.query_service,
        chrono::Duration::days(3),
    )
    .await
    .unwrap();
    assert_eq!(recovered, 1);

    scouter_drift::genai::test_helpers::drain_once(&helper.pool)
        .await
        .unwrap();
    wait_for_status(&helper.pool, &record_uid, "pending").await;
}
