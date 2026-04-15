use crate::common::setup_test;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use http_body_util::BodyExt;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::tonic::common::v1::{any_value, AnyValue, KeyValue};
use opentelemetry_proto::tonic::resource::v1::Resource;
use opentelemetry_proto::tonic::trace::v1::{ResourceSpans, ScopeSpans, Span};
use potato_head::create_uuid7;
use scouter_sql::sql::aggregator::shutdown_trace_cache;
use scouter_types::{MessageRecord, TagRecord, TagsRequest, TagsResponse, TraceServerRecord};
use scouter_types::{sql::TraceFilters, TracePaginationResponse};
use std::time::Duration;
use tokio::time::sleep;

fn make_trace_server_record() -> MessageRecord {
    let trace_id: Vec<u8> = (0u8..16).collect();
    let span_id: Vec<u8> = (0u8..8).collect();

    let service_name_kv = KeyValue {
        key: "service.name".to_string(),
        value: Some(AnyValue {
            value: Some(any_value::Value::StringValue("test-service".to_string())),
        }),
    };

    let span = Span {
        trace_id: trace_id.clone(),
        span_id: span_id.clone(),
        name: "test-span".to_string(),
        start_time_unix_nano: 1_000_000_000,
        end_time_unix_nano: 2_000_000_000,
        ..Default::default()
    };

    let scope_spans = ScopeSpans {
        spans: vec![span],
        ..Default::default()
    };

    let resource = Resource {
        attributes: vec![service_name_kv],
        ..Default::default()
    };

    let resource_spans = ResourceSpans {
        resource: Some(resource),
        scope_spans: vec![scope_spans],
        ..Default::default()
    };

    MessageRecord::TraceServerRecord(TraceServerRecord {
        request: ExportTraceServiceRequest {
            resource_spans: vec![resource_spans],
        },
    })
}

#[tokio::test]
async fn test_trace_ingest_via_message_channel() {
    let helper = setup_test().await;

    let record = make_trace_server_record();
    let body = serde_json::to_string(&record).unwrap();

    let request = Request::builder()
        .uri("/scouter/message")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Allow trace worker time to process and Delta Lake write to complete
    sleep(Duration::from_secs(3)).await;

    // Flush the in-memory trace cache so summary records are visible to the paginated query
    let _ = shutdown_trace_cache(&helper.pool).await;

    // Verify trace appears in the paginated trace endpoint
    let filters = TraceFilters {
        limit: Some(10),
        ..Default::default()
    };
    let body = serde_json::to_string(&filters).unwrap();
    let request = Request::builder()
        .uri("/scouter/trace/paginated")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let page: TracePaginationResponse = serde_json::from_slice(&bytes).unwrap();
    assert!(!page.items.is_empty(), "Trace record should appear after channel ingest");
}

#[tokio::test]
async fn test_tag_ingest_via_message_channel() {
    let helper = setup_test().await;

    let entity_id = create_uuid7();
    let tag = TagRecord {
        entity_id: entity_id.clone(),
        entity_type: "model".to_string(),
        key: "env".to_string(),
        value: "staging".to_string(),
    };

    let record = MessageRecord::TagServerRecord(tag);
    let body = serde_json::to_string(&record).unwrap();

    let request = Request::builder()
        .uri("/scouter/message")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Allow tag worker time to write to DB
    sleep(Duration::from_secs(2)).await;

    // Verify tag is queryable
    let tag_request = TagsRequest {
        entity_type: "model".to_string(),
        entity_id: entity_id.clone(),
    };
    let query_string = serde_qs::to_string(&tag_request).unwrap();
    let request = Request::builder()
        .uri(format!("/scouter/tags?{query_string}"))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let tags_response: TagsResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(tags_response.tags.len(), 1, "Tag should be present after channel ingest");
    assert_eq!(tags_response.tags[0].key, "env");
}

#[tokio::test]
async fn test_trace_ingest_via_drift_endpoint() {
    let helper = setup_test().await;

    let record = make_trace_server_record();
    let body = serde_json::to_string(&record).unwrap();

    let request = Request::builder()
        .uri("/scouter/drift")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);

    sleep(Duration::from_secs(3)).await;
    let _ = shutdown_trace_cache(&helper.pool).await;

    let filters = TraceFilters {
        limit: Some(10),
        ..Default::default()
    };
    let body = serde_json::to_string(&filters).unwrap();
    let request = Request::builder()
        .uri("/scouter/trace/paginated")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let page: TracePaginationResponse = serde_json::from_slice(&bytes).unwrap();
    assert!(!page.items.is_empty(), "Trace record should appear via /scouter/drift ingest");
}

#[tokio::test]
async fn test_tag_ingest_via_drift_endpoint() {
    let helper = setup_test().await;

    let entity_id = create_uuid7();
    let tag = TagRecord {
        entity_id: entity_id.clone(),
        entity_type: "model".to_string(),
        key: "region".to_string(),
        value: "us-east-1".to_string(),
    };

    let record = MessageRecord::TagServerRecord(tag);
    let body = serde_json::to_string(&record).unwrap();

    let request = Request::builder()
        .uri("/scouter/drift")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);

    sleep(Duration::from_secs(2)).await;

    let tag_request = TagsRequest {
        entity_type: "model".to_string(),
        entity_id: entity_id.clone(),
    };
    let query_string = serde_qs::to_string(&tag_request).unwrap();
    let request = Request::builder()
        .uri(format!("/scouter/tags?{query_string}"))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let tags_response: TagsResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(tags_response.tags.len(), 1, "Tag should be present via /scouter/drift ingest");
}
