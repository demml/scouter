use crate::common::{TestHelper, setup_test};
use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use chrono::{DateTime, Utc};
use http_body_util::BodyExt;
use scouter_types::{GenAiSpanRecord, SpanId, TraceId};
use std::time::{Duration, Instant};

fn build_genai_span(
    trace_id: TraceId,
    span_id: u8,
    start_time: DateTime<Utc>,
    agent_name: Option<&str>,
) -> GenAiSpanRecord {
    GenAiSpanRecord {
        trace_id,
        span_id: SpanId::from_bytes([span_id; 8]),
        service_name: "genai-test-service".to_string(),
        start_time,
        end_time: Some(start_time + chrono::Duration::milliseconds(120)),
        duration_ms: 120,
        status_code: 0,
        operation_name: Some("invoke_agent".to_string()),
        provider_name: Some("anthropic".to_string()),
        request_model: Some("claude-opus-4-7".to_string()),
        response_model: Some("claude-opus-4-7".to_string()),
        input_tokens: Some(50),
        output_tokens: Some(80),
        agent_name: agent_name.map(|s| s.to_string()),
        conversation_id: Some("conv-test-123".to_string()),
        ..Default::default()
    }
}

async fn wait_for_genai_spans(
    helper: &TestHelper,
    trace_id: &TraceId,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    expected_count: usize,
) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let result = helper
            .genai_service
            .query_service
            .get_genai_spans_by_trace_id(trace_id, start_time, end_time, 10_000)
            .await
            .unwrap();
        if result.spans.len() >= expected_count {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "Timed out waiting for {expected_count} genai spans, saw {}",
            result.spans.len()
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn test_genai_dashboard_empty() {
    let helper = setup_test().await;
    let now = Utc::now();

    let body = serde_json::json!({
        "start_time": (now - chrono::Duration::hours(1)).to_rfc3339(),
        "end_time": (now + chrono::Duration::hours(1)).to_rfc3339(),
        "bucket_interval": "hour",
        "model_pricing": {}
    });

    let request = Request::builder()
        .uri("/scouter/genai/dashboard")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(json["metadata"]["total_spans"], 0);
    assert_eq!(json["metadata"]["schema_version"], 1);
    assert_eq!(json["buckets_truncated"], false);
    assert!(
        json["token_metrics"]["buckets"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        json["operation_breakdown"]["operations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        json["agent_dashboard"]["buckets"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(json["applied_filters"]["bucket_interval"], "hour");
}

#[tokio::test]
async fn test_genai_dashboard_with_data() {
    let helper = setup_test().await;
    let now = Utc::now();
    let trace_id = TraceId::from_bytes([80u8; 16]);

    let records: Vec<GenAiSpanRecord> = (0..3)
        .map(|i| {
            build_genai_span(
                trace_id,
                i + 1,
                now + chrono::Duration::seconds(i as i64),
                Some("test-agent"),
            )
        })
        .collect();
    helper.genai_service.write_records(records).await.unwrap();

    wait_for_genai_spans(
        &helper,
        &trace_id,
        now - chrono::Duration::minutes(5),
        now + chrono::Duration::hours(1),
        3,
    )
    .await;

    let body = serde_json::json!({
        "start_time": (now - chrono::Duration::hours(1)).to_rfc3339(),
        "end_time": (now + chrono::Duration::hours(1)).to_rfc3339(),
        "bucket_interval": "hour",
        "model_pricing": {}
    });

    let request = Request::builder()
        .uri("/scouter/genai/dashboard")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    assert!(json["metadata"]["total_spans"].as_i64().unwrap() >= 3);
    assert!(
        !json["token_metrics"]["buckets"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        !json["operation_breakdown"]["operations"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        !json["agent_dashboard"]["buckets"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let summary = &json["agent_dashboard"]["summary"];
    assert!(summary["total_requests"].as_i64().unwrap() >= 3);
    assert!(summary["total_input_tokens"].as_i64().unwrap() >= 150);
    assert!(summary["total_output_tokens"].as_i64().unwrap() >= 240);
    assert_eq!(summary["unique_agent_count"], 1);

    let applied = &json["applied_filters"];
    assert_eq!(applied["bucket_interval"], "hour");
    assert!(applied["service_name"].is_null());
}

#[tokio::test]
async fn test_genai_dashboard_with_service_filter() {
    let helper = setup_test().await;
    let now = Utc::now();
    let trace_id = TraceId::from_bytes([81u8; 16]);

    let records: Vec<GenAiSpanRecord> = (0..2)
        .map(|i| {
            build_genai_span(
                trace_id,
                i + 1,
                now + chrono::Duration::seconds(i as i64),
                None,
            )
        })
        .collect();
    helper.genai_service.write_records(records).await.unwrap();

    wait_for_genai_spans(
        &helper,
        &trace_id,
        now - chrono::Duration::minutes(5),
        now + chrono::Duration::hours(1),
        2,
    )
    .await;

    let body_matching = serde_json::json!({
        "service_name": "genai-test-service",
        "start_time": (now - chrono::Duration::hours(1)).to_rfc3339(),
        "end_time": (now + chrono::Duration::hours(1)).to_rfc3339(),
        "bucket_interval": "hour",
        "model_pricing": {}
    });
    let request = Request::builder()
        .uri("/scouter/genai/dashboard")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body_matching).unwrap()))
        .unwrap();
    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json_match: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(json_match["metadata"]["total_spans"].as_i64().unwrap() >= 2);

    let body_no_match = serde_json::json!({
        "service_name": "nonexistent-service",
        "start_time": (now - chrono::Duration::hours(1)).to_rfc3339(),
        "end_time": (now + chrono::Duration::hours(1)).to_rfc3339(),
        "bucket_interval": "hour",
        "model_pricing": {}
    });
    let request = Request::builder()
        .uri("/scouter/genai/dashboard")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body_no_match).unwrap()))
        .unwrap();
    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json_no_match: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json_no_match["metadata"]["total_spans"], 0);
}

#[tokio::test]
async fn test_genai_dashboard_validation_errors() {
    let helper = setup_test().await;
    let now = Utc::now();

    // Invalid bucket_interval
    let request = Request::builder()
        .uri("/scouter/genai/dashboard")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "start_time": (now - chrono::Duration::hours(1)).to_rfc3339(),
                "end_time": now.to_rfc3339(),
                "bucket_interval": "fortnight"
            })
            .to_string(),
        ))
        .unwrap();
    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // start_time == end_time
    let request = Request::builder()
        .uri("/scouter/genai/dashboard")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "start_time": now.to_rfc3339(),
                "end_time": now.to_rfc3339(),
                "bucket_interval": "hour"
            })
            .to_string(),
        ))
        .unwrap();
    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // Window exceeds 30 days
    let request = Request::builder()
        .uri("/scouter/genai/dashboard")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "start_time": (now - chrono::Duration::days(31)).to_rfc3339(),
                "end_time": now.to_rfc3339(),
                "bucket_interval": "day"
            })
            .to_string(),
        ))
        .unwrap();
    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // start_time after end_time
    let request = Request::builder()
        .uri("/scouter/genai/dashboard")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "start_time": now.to_rfc3339(),
                "end_time": (now - chrono::Duration::hours(1)).to_rfc3339(),
                "bucket_interval": "hour"
            })
            .to_string(),
        ))
        .unwrap();
    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    // entity_id exceeds 256 characters
    let request = Request::builder()
        .uri("/scouter/genai/dashboard")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "start_time": (now - chrono::Duration::hours(1)).to_rfc3339(),
                "end_time": now.to_rfc3339(),
                "bucket_interval": "hour",
                "entity_id": "x".repeat(257)
            })
            .to_string(),
        ))
        .unwrap();
    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(
        json["error"].as_str().unwrap_or("").contains("entity_id"),
        "error should mention entity_id"
    );
}

#[tokio::test]
async fn test_agent_dashboard_empty() {
    let helper = setup_test().await;
    let now = Utc::now();

    let body = serde_json::json!({
        "start_time": (now - chrono::Duration::hours(1)).to_rfc3339(),
        "end_time": (now + chrono::Duration::hours(1)).to_rfc3339(),
        "bucket_interval": "hour",
        "model_pricing": {}
    });

    let request = Request::builder()
        .uri("/scouter/genai/agent/metrics")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(json["summary"]["total_requests"], 0);
    assert_eq!(json["summary"]["unique_agent_count"], 0);
    assert!(json["buckets"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_agent_dashboard_with_data() {
    let helper = setup_test().await;
    let now = Utc::now();
    let trace_id = TraceId::from_bytes([82u8; 16]);

    let records: Vec<GenAiSpanRecord> = (0..4)
        .map(|i| {
            build_genai_span(
                trace_id,
                i + 1,
                now + chrono::Duration::seconds(i as i64),
                Some("dashboard-agent"),
            )
        })
        .collect();
    helper.genai_service.write_records(records).await.unwrap();

    wait_for_genai_spans(
        &helper,
        &trace_id,
        now - chrono::Duration::minutes(5),
        now + chrono::Duration::hours(1),
        4,
    )
    .await;

    let body = serde_json::json!({
        "start_time": (now - chrono::Duration::hours(1)).to_rfc3339(),
        "end_time": (now + chrono::Duration::hours(1)).to_rfc3339(),
        "bucket_interval": "hour",
        "agent_name": "dashboard-agent",
        "model_pricing": {}
    });

    let request = Request::builder()
        .uri("/scouter/genai/agent/metrics")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    let summary = &json["summary"];
    assert!(summary["total_requests"].as_i64().unwrap() >= 4);
    assert!(summary["total_input_tokens"].as_i64().unwrap() >= 200);
    assert_eq!(summary["overall_error_rate"], 0.0);
    assert!(summary["avg_duration_ms"].as_f64().unwrap() > 0.0);

    let buckets = json["buckets"].as_array().unwrap();
    assert!(!buckets.is_empty());
    for bucket in buckets {
        assert!(bucket["span_count"].as_i64().unwrap() > 0);
        assert!(bucket["avg_duration_ms"].as_f64().unwrap() > 0.0);
    }
}

#[tokio::test]
async fn test_agent_dashboard_cost_by_model() {
    let helper = setup_test().await;
    let now = Utc::now();
    let trace_id = TraceId::from_bytes([83u8; 16]);

    let records: Vec<GenAiSpanRecord> = (0..2)
        .map(|i| {
            build_genai_span(
                trace_id,
                i + 1,
                now + chrono::Duration::seconds(i as i64),
                None,
            )
        })
        .collect();
    helper.genai_service.write_records(records).await.unwrap();

    wait_for_genai_spans(
        &helper,
        &trace_id,
        now - chrono::Duration::minutes(5),
        now + chrono::Duration::hours(1),
        2,
    )
    .await;

    let body = serde_json::json!({
        "start_time": (now - chrono::Duration::hours(1)).to_rfc3339(),
        "end_time": (now + chrono::Duration::hours(1)).to_rfc3339(),
        "bucket_interval": "hour",
        "model_pricing": {
            "claude-opus-4-7": {
                "input_per_million": 15.0,
                "output_per_million": 75.0,
                "cache_creation_per_million": 18.75,
                "cache_read_per_million": 1.5
            }
        }
    });

    let request = Request::builder()
        .uri("/scouter/genai/agent/metrics")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    let cost_by_model = json["summary"]["cost_by_model"].as_array().unwrap();
    assert!(
        !cost_by_model.is_empty(),
        "Expected cost breakdown when pricing supplied"
    );

    let model_entry = cost_by_model
        .iter()
        .find(|e| e["model"] == "claude-opus-4-7")
        .expect("Expected claude-opus-4-7 in cost_by_model");

    assert!(
        model_entry["total_cost"].as_f64().is_some(),
        "Expected computed cost"
    );
    assert!(model_entry["total_cost"].as_f64().unwrap() > 0.0);
    assert!(model_entry["total_input_tokens"].as_i64().unwrap() >= 100);
    assert!(model_entry["total_output_tokens"].as_i64().unwrap() >= 160);
}

#[tokio::test]
async fn test_genai_dashboard_filter_params() {
    let helper = setup_test().await;
    let now = Utc::now();
    let trace_id = TraceId::from_bytes([84u8; 16]);

    let records: Vec<GenAiSpanRecord> = (0..3)
        .map(|i| {
            build_genai_span(
                trace_id,
                i + 1,
                now + chrono::Duration::seconds(i as i64),
                Some("filter-agent"),
            )
        })
        .collect();
    helper.genai_service.write_records(records).await.unwrap();

    wait_for_genai_spans(
        &helper,
        &trace_id,
        now - chrono::Duration::minutes(5),
        now + chrono::Duration::hours(1),
        3,
    )
    .await;

    // agent_name filter should narrow results to matching spans
    let body = serde_json::json!({
        "start_time": (now - chrono::Duration::hours(1)).to_rfc3339(),
        "end_time": (now + chrono::Duration::hours(1)).to_rfc3339(),
        "bucket_interval": "hour",
        "agent_name": "filter-agent",
        "model_pricing": {}
    });
    let request = Request::builder()
        .uri("/scouter/genai/dashboard")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(
        json["metadata"]["total_spans"].as_i64().unwrap() >= 3,
        "agent_name filter should include matching spans"
    );

    // nonexistent model filter should yield zero spans
    let body_no_match = serde_json::json!({
        "start_time": (now - chrono::Duration::hours(1)).to_rfc3339(),
        "end_time": (now + chrono::Duration::hours(1)).to_rfc3339(),
        "bucket_interval": "hour",
        "model": "nonexistent-model",
        "model_pricing": {}
    });
    let request = Request::builder()
        .uri("/scouter/genai/dashboard")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body_no_match).unwrap()))
        .unwrap();
    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json_no_match: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        json_no_match["metadata"]["total_spans"], 0,
        "nonexistent model filter should return zero spans"
    );
}

#[tokio::test]
async fn test_agent_dashboard_agent_name_filter_no_match() {
    let helper = setup_test().await;
    let now = Utc::now();
    let trace_id = TraceId::from_bytes([85u8; 16]);

    let records: Vec<GenAiSpanRecord> = (0..2)
        .map(|i| {
            build_genai_span(
                trace_id,
                i + 1,
                now + chrono::Duration::seconds(i as i64),
                Some("present-agent"),
            )
        })
        .collect();
    helper.genai_service.write_records(records).await.unwrap();

    wait_for_genai_spans(
        &helper,
        &trace_id,
        now - chrono::Duration::minutes(5),
        now + chrono::Duration::hours(1),
        2,
    )
    .await;

    let body = serde_json::json!({
        "start_time": (now - chrono::Duration::hours(1)).to_rfc3339(),
        "end_time": (now + chrono::Duration::hours(1)).to_rfc3339(),
        "bucket_interval": "hour",
        "agent_name": "other-agent",
        "model_pricing": {}
    });
    let request = Request::builder()
        .uri("/scouter/genai/agent/metrics")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(
        json["summary"]["total_requests"], 0,
        "agent_name filter for absent agent should yield zero requests"
    );
    assert!(
        json["buckets"].as_array().unwrap().is_empty(),
        "agent_name filter for absent agent should yield empty buckets"
    );
}

#[tokio::test]
async fn test_genai_dashboard_entity_id_filter() {
    let helper = setup_test().await;
    let now = Utc::now();
    let trace_id = TraceId::from_bytes([90u8; 16]);

    // 3 spans tagged with entity-A
    let entity_records: Vec<GenAiSpanRecord> = (0..3)
        .map(|i| {
            let mut r = build_genai_span(
                trace_id,
                i + 1,
                now + chrono::Duration::seconds(i as i64),
                Some("entity-agent"),
            );
            r.entity_id = Some("entity-A".to_string());
            r
        })
        .collect();

    // 2 spans with no entity_id
    let other_records: Vec<GenAiSpanRecord> = (0..2)
        .map(|i| {
            build_genai_span(
                trace_id,
                i + 10,
                now + chrono::Duration::seconds(10 + i as i64),
                None,
            )
        })
        .collect();

    let mut all_records = entity_records;
    all_records.extend(other_records);
    helper
        .genai_service
        .write_records(all_records)
        .await
        .unwrap();

    wait_for_genai_spans(
        &helper,
        &trace_id,
        now - chrono::Duration::minutes(5),
        now + chrono::Duration::hours(1),
        5,
    )
    .await;

    // entity_id filter: should return only 3 matching spans
    let body = serde_json::json!({
        "start_time": (now - chrono::Duration::hours(1)).to_rfc3339(),
        "end_time": (now + chrono::Duration::hours(1)).to_rfc3339(),
        "bucket_interval": "hour",
        "entity_id": "entity-A",
        "model_pricing": {}
    });
    let request = Request::builder()
        .uri("/scouter/genai/dashboard")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        json["metadata"]["total_spans"], 3,
        "entity_id filter should return only entity-A spans"
    );

    // nonexistent entity_id should return zero spans
    let body_no_match = serde_json::json!({
        "start_time": (now - chrono::Duration::hours(1)).to_rfc3339(),
        "end_time": (now + chrono::Duration::hours(1)).to_rfc3339(),
        "bucket_interval": "hour",
        "entity_id": "entity-nonexistent",
        "model_pricing": {}
    });
    let request = Request::builder()
        .uri("/scouter/genai/dashboard")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body_no_match).unwrap()))
        .unwrap();
    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json_no_match: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        json_no_match["metadata"]["total_spans"], 0,
        "nonexistent entity_id should return zero spans"
    );
}
