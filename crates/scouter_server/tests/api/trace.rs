use crate::common::setup_test;
use crate::common::TestHelper;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use chrono::Utc;
use http_body_util::BodyExt;
use scouter_sql::sql::aggregator::shutdown_trace_cache;
use scouter_types::{
    sql::TraceFilters, GenAiSpanRecord, SpanId, SpansFromTagsRequest, TraceId, TraceMetricsRequest,
    TraceMetricsResponse, TracePaginationResponse, TraceRequest, TraceSpansResponse,
};
use std::collections::{HashMap, HashSet};

async fn fetch_paginated(helper: &TestHelper, filters: &TraceFilters) -> TracePaginationResponse {
    let body = serde_json::to_string(filters).unwrap();
    let request = Request::builder()
        .uri("/scouter/trace/paginated")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();
    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

async fn fetch_spans_from_filters(
    helper: &TestHelper,
    filters: &TraceFilters,
) -> TraceSpansResponse {
    let body = serde_json::to_string(filters).unwrap();
    let request = Request::builder()
        .uri("/scouter/trace/spans/filters")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();
    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

async fn fetch_metrics(helper: &TestHelper, request: &TraceMetricsRequest) -> TraceMetricsResponse {
    let req = Request::builder()
        .uri("/scouter/trace/metrics")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(request).unwrap()))
        .unwrap();
    let response = helper.send_oneshot(req).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

fn build_genai_span(
    trace_id: TraceId,
    span_id: u8,
    start_time: chrono::DateTime<Utc>,
) -> GenAiSpanRecord {
    GenAiSpanRecord {
        trace_id,
        span_id: SpanId::from_bytes([span_id; 8]),
        service_name: "genai-service".to_string(),
        start_time,
        end_time: Some(start_time + chrono::Duration::milliseconds(80)),
        duration_ms: 80,
        status_code: 0,
        operation_name: Some("invoke_agent".to_string()),
        provider_name: Some("openai".to_string()),
        request_model: Some("gpt-4".to_string()),
        response_model: Some("gpt-4o".to_string()),
        input_tokens: Some(12),
        output_tokens: Some(20),
        input_messages: Some(r#"[{"role":"user","content":"hi"}]"#.to_string()),
        output_messages: Some(r#"[{"role":"assistant","content":"hello"}]"#.to_string()),
        system_instructions: Some("be concise".to_string()),
        tool_definitions: Some(r#"[{"name":"search"}]"#.to_string()),
        ..Default::default()
    }
}

#[tokio::test]
async fn test_tracing() {
    let helper = setup_test().await;
    helper.generate_trace_data().await.unwrap();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let _flushed = shutdown_trace_cache(&helper.pool).await.unwrap();

    // Fetch a single page to get records for subsequent tests
    let filters = TraceFilters {
        limit: Some(50),
        ..Default::default()
    };
    let first_batch = fetch_paginated(&helper, &filters).await;
    assert!(!first_batch.items.is_empty(), "Should have trace records");
    let first_trace_id = &first_batch.items.first().unwrap().trace_id;

    let filtered_record = first_batch
        .items
        .iter()
        .find(|record| record.span_count > 5)
        .unwrap();

    // now get spans for one of the traces
    let params = TraceRequest {
        trace_id: filtered_record.trace_id.clone(),
        service_name: None,
        start_time: None,
        end_time: None,
    };

    let query_string = serde_qs::to_string(&params).unwrap();
    let request = Request::builder()
        .uri(format!("/scouter/trace/spans?{query_string}"))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let spans: TraceSpansResponse = serde_json::from_slice(&body).unwrap();

    assert!(
        !spans.spans.is_empty(),
        "Should return spans for the specified trace"
    );

    // send same request to get trace baggage
    let request = Request::builder()
        .uri(format!("/scouter/trace/baggage?{query_string}"))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let start_time = filtered_record.start_time - chrono::Duration::hours(24);
    let end_time = filtered_record.start_time + chrono::Duration::minutes(5);

    // make request for trace metrics
    let metrics_request = TraceMetricsRequest {
        service_name: None,
        start_time,
        end_time,
        bucket_interval: "hour".to_string(),
        attribute_filters: None,
        entity_uid: None,
        duration_min_ms: None,
        duration_max_ms: None,
    };

    let request = Request::builder()
        .uri("/scouter/trace/metrics".to_string())
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&metrics_request).unwrap()))
        .unwrap();

    // assert we have data points
    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let metrics_response: TraceMetricsResponse = serde_json::from_slice(&body).unwrap();

    assert!(!metrics_response.metrics.is_empty());

    // get trace by tags
    let mut map = HashMap::new();
    map.insert("key".to_string(), "scouter.queue.record".to_string());
    map.insert("value".to_string(), first_trace_id.clone());

    let trace_request = SpansFromTagsRequest {
        entity_type: "trace".to_string(),
        tag_filters: vec![map],
        match_all: false,
        service_name: None,
    };

    let request = Request::builder()
        .uri("/scouter/trace/spans/tags".to_string())
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&trace_request).unwrap()))
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let spans_response: TraceSpansResponse = serde_json::from_slice(&body).unwrap();
    assert!(
        !spans_response.spans.is_empty(),
        "Should return spans for the specified tags"
    );

    // Attribute filter: tests the DataFusion JOIN path (component=kafka spans)
    let attr_filters = TraceFilters {
        attribute_filters: Some(vec!["component=kafka".to_string()]),
        ..Default::default()
    };
    let attr_batch = fetch_paginated(&helper, &attr_filters).await;
    assert!(
        !attr_batch.items.is_empty(),
        "Should return records with attribute filter"
    );
}

#[tokio::test]
async fn test_trace_pagination() {
    let helper = setup_test().await;
    helper.generate_trace_data().await.unwrap();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let _flushed = shutdown_trace_cache(&helper.pool).await.unwrap();

    // Forward walk with limit=30: expect pages of 30/30/30/10
    let mut filters = TraceFilters {
        limit: Some(30),
        ..Default::default()
    };

    let page = fetch_paginated(&helper, &filters).await;
    assert_eq!(page.items.len(), 30, "First page should have 30 items");
    assert!(page.has_next, "First page should have next");
    assert!(!page.has_previous, "First page should not have previous");

    let mut forward_ids: HashSet<String> = HashSet::new();
    let mut prev_page_ids: HashSet<String> = page
        .items
        .iter()
        .map(|item| item.trace_id.clone())
        .collect();
    forward_ids.extend(prev_page_ids.iter().cloned());

    let mut last_page = page;
    let mut page_count = 1;

    // Walk forward through remaining pages
    while last_page.has_next {
        let cursor = last_page.next_cursor.as_ref().unwrap();
        filters = filters.next_page(cursor);
        let page = fetch_paginated(&helper, &filters).await;

        assert!(!page.items.is_empty(), "Page should not be empty");
        assert!(page.has_previous, "Non-first page should have previous");

        let current_page_ids: HashSet<String> = page
            .items
            .iter()
            .map(|item| item.trace_id.clone())
            .collect();

        // No overlap with previous page
        let overlap: HashSet<_> = current_page_ids.intersection(&prev_page_ids).collect();
        assert!(
            overlap.is_empty(),
            "Page {} should not overlap with previous page, found {:?}",
            page_count + 1,
            overlap
        );

        forward_ids.extend(current_page_ids.iter().cloned());
        prev_page_ids = current_page_ids;
        last_page = page;
        page_count += 1;
    }

    // Verify totals
    assert_eq!(forward_ids.len(), 100, "Should have 100 unique trace_ids");
    assert_eq!(last_page.items.len(), 10, "Last page should have 10 items");
    assert!(!last_page.has_next, "Last page should not have next");

    // Backward walk: start from the last forward page's previous_cursor
    let mut backward_ids: HashSet<String> = last_page
        .items
        .iter()
        .map(|item| item.trace_id.clone())
        .collect();
    let mut prev_page_ids: HashSet<String> = backward_ids.clone();

    let mut current_page = last_page;

    while current_page.has_previous {
        let cursor = current_page.previous_cursor.as_ref().unwrap();
        filters = filters.previous_page(cursor);
        let page = fetch_paginated(&helper, &filters).await;

        assert!(!page.items.is_empty(), "Backward page should not be empty");
        assert!(page.has_next, "Non-last backward page should have next");

        let current_page_ids: HashSet<String> = page
            .items
            .iter()
            .map(|item| item.trace_id.clone())
            .collect();

        // No overlap with the page we just came from
        let overlap: HashSet<_> = current_page_ids.intersection(&prev_page_ids).collect();
        assert!(
            overlap.is_empty(),
            "Backward page should not overlap with previous page, found {:?}",
            overlap
        );

        backward_ids.extend(current_page_ids.iter().cloned());
        prev_page_ids = current_page_ids;
        current_page = page;
    }

    assert!(
        !current_page.has_previous,
        "First backward page should not have previous"
    );
    assert_eq!(
        backward_ids.len(),
        100,
        "Backward walk should cover all 100 trace_ids"
    );
    assert_eq!(
        backward_ids, forward_ids,
        "Backward walk should cover the same trace_ids as forward walk"
    );
}

#[tokio::test]
async fn test_paginated_traces_duration_min() {
    let helper = setup_test().await;
    helper
        .generate_traces_with_durations(&[50, 200, 800, 1500])
        .await
        .unwrap();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let _ = shutdown_trace_cache(&helper.pool).await.unwrap();

    let filters = TraceFilters {
        duration_min_ms: Some(500),
        limit: Some(50),
        ..Default::default()
    };
    let page = fetch_paginated(&helper, &filters).await;

    assert_eq!(
        page.items.len(),
        2,
        "Expected 2 traces with duration >= 500ms"
    );
    for item in &page.items {
        assert!(item.duration_ms.unwrap_or(0) >= 500);
    }
}

#[tokio::test]
async fn test_paginated_traces_duration_max() {
    let helper = setup_test().await;
    helper
        .generate_traces_with_durations(&[50, 200, 800, 1500])
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let _ = shutdown_trace_cache(&helper.pool).await.unwrap();

    let filters = TraceFilters {
        duration_max_ms: Some(300),
        limit: Some(50),
        ..Default::default()
    };
    let page = fetch_paginated(&helper, &filters).await;

    assert_eq!(page.items.len(), 2);
    for item in &page.items {
        assert!(item.duration_ms.unwrap_or(i64::MAX) <= 300);
    }
}

#[tokio::test]
async fn test_paginated_traces_duration_range_inclusive() {
    let helper = setup_test().await;
    helper
        .generate_traces_with_durations(&[50, 100, 300, 500, 1000])
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let _ = shutdown_trace_cache(&helper.pool).await.unwrap();

    let filters = TraceFilters {
        duration_min_ms: Some(100),
        duration_max_ms: Some(500),
        limit: Some(50),
        ..Default::default()
    };
    let page = fetch_paginated(&helper, &filters).await;

    assert_eq!(page.items.len(), 3, "Expected 3 traces in [100, 500]");
    let durations: Vec<i64> = page.items.iter().filter_map(|i| i.duration_ms).collect();
    assert!(durations.contains(&100));
    assert!(durations.contains(&500));
}

#[tokio::test]
async fn test_trace_metrics_duration_filter() {
    let helper = setup_test().await;
    helper
        .generate_traces_with_durations(&[50, 200, 800, 1500])
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let _ = shutdown_trace_cache(&helper.pool).await.unwrap();

    let now = Utc::now();
    let unfiltered = fetch_metrics(
        &helper,
        &TraceMetricsRequest {
            service_name: None,
            start_time: now - chrono::Duration::hours(2),
            end_time: now + chrono::Duration::hours(1),
            bucket_interval: "hour".to_string(),
            attribute_filters: None,
            entity_uid: None,
            duration_min_ms: None,
            duration_max_ms: None,
        },
    )
    .await;
    let unfiltered_total: i64 = unfiltered.metrics.iter().map(|m| m.trace_count).sum();

    let filtered = fetch_metrics(
        &helper,
        &TraceMetricsRequest {
            service_name: None,
            start_time: now - chrono::Duration::hours(2),
            end_time: now + chrono::Duration::hours(1),
            bucket_interval: "hour".to_string(),
            attribute_filters: None,
            entity_uid: None,
            duration_min_ms: Some(500),
            duration_max_ms: None,
        },
    )
    .await;
    let filtered_total: i64 = filtered.metrics.iter().map(|m| m.trace_count).sum();

    assert!(
        filtered_total < unfiltered_total,
        "Duration filter should shrink count"
    );
    assert_eq!(filtered_total, 2);
    for bucket in &filtered.metrics {
        assert!(bucket.avg_duration_ms >= 500.0);
    }
}

#[tokio::test]
async fn test_spans_from_filters_duration() {
    let helper = setup_test().await;
    helper
        .generate_traces_with_durations(&[100, 1500])
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let _ = shutdown_trace_cache(&helper.pool).await.unwrap();

    let filters = TraceFilters {
        duration_min_ms: Some(1000),
        ..Default::default()
    };
    let response = fetch_spans_from_filters(&helper, &filters).await;

    assert!(
        !response.spans.is_empty(),
        "Expected spans for the slow trace"
    );
    let trace_ids: HashSet<_> = response.spans.iter().map(|s| s.trace_id.clone()).collect();
    assert_eq!(
        trace_ids.len(),
        1,
        "spans-from-filters returns one trace at a time"
    );
}

#[tokio::test]
async fn test_paginated_traces_inverted_range_empty() {
    let helper = setup_test().await;
    helper
        .generate_traces_with_durations(&[100, 200, 300])
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let _ = shutdown_trace_cache(&helper.pool).await.unwrap();

    let filters = TraceFilters {
        duration_min_ms: Some(500),
        duration_max_ms: Some(100),
        ..Default::default()
    };
    let page = fetch_paginated(&helper, &filters).await;
    assert_eq!(page.items.len(), 0);
}

#[tokio::test]
async fn test_genai_trace_metrics_route() {
    let helper = setup_test().await;
    let trace_id = TraceId::from_bytes([55u8; 16]);
    let other_trace_id = TraceId::from_bytes([56u8; 16]);
    let now = Utc::now();

    let records = vec![
        build_genai_span(trace_id, 1, now),
        build_genai_span(other_trace_id, 2, now),
    ];
    helper.genai_service.write_records(records).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(4)).await;

    let body = serde_json::json!({
        "start_time": (now - chrono::Duration::hours(1)).to_rfc3339(),
        "end_time": (now + chrono::Duration::hours(1)).to_rfc3339(),
        "bucket_interval": "hour",
        "model_pricing": {},
        "span_limit": 10,
        "include_sensitive_content": false
    });

    let request = Request::builder()
        .uri(format!(
            "/scouter/genai/traces/{}/metrics",
            trace_id.to_hex()
        ))
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(response_json["trace_id"], trace_id.to_hex());
    assert_eq!(response_json["span_limit"], 10);
    assert_eq!(response_json["spans_truncated"], false);
    assert_eq!(response_json["sensitive_content_redacted"], true);
    let spans = response_json["spans"].as_array().unwrap();
    assert_eq!(
        spans.len(),
        1,
        "Only spans for requested trace should be returned"
    );
    assert_eq!(spans[0]["trace_id"], trace_id.to_hex());
    assert!(spans[0]["input_messages"].is_null());
    assert!(spans[0]["output_messages"].is_null());
    assert!(spans[0]["system_instructions"].is_null());
    assert!(spans[0]["tool_definitions"].is_null());
}

#[tokio::test]
async fn test_genai_trace_metrics_route_validation_errors() {
    let helper = setup_test().await;

    let invalid_trace_request = Request::builder()
        .uri("/scouter/genai/traces/not-a-trace-id/metrics")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "bucket_interval": "hour"
            })
            .to_string(),
        ))
        .unwrap();
    let invalid_trace_response = helper.send_oneshot(invalid_trace_request).await;
    assert_eq!(invalid_trace_response.status(), StatusCode::BAD_REQUEST);

    let valid_trace_id = TraceId::from_bytes([57u8; 16]).to_hex();
    let invalid_bucket_request = Request::builder()
        .uri(format!("/scouter/genai/traces/{valid_trace_id}/metrics"))
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "bucket_interval": "fortnight"
            })
            .to_string(),
        ))
        .unwrap();
    let invalid_bucket_response = helper.send_oneshot(invalid_bucket_request).await;
    assert_eq!(invalid_bucket_response.status(), StatusCode::BAD_REQUEST);
}
