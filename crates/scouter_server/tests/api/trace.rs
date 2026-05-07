use crate::common::TestHelper;
use crate::common::setup_test;
use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use chrono::{DateTime, Utc};
use http_body_util::BodyExt;
use scouter_server::api::routes::user::schema::CreateUserRequest;
use scouter_sql::sql::aggregator::shutdown_trace_cache;
use scouter_types::{
    GenAiSpanRecord, SpanId, SpansFromTagsRequest, TraceFacetsResponse, TraceId,
    TraceMetricsRequest, TraceMetricsResponse, TracePaginationResponse, TraceRequest,
    TraceSpansResponse, sql::TraceFilters, trace::query::FilterClause,
};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

fn with_q(path: &str, q: &str) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    serializer.append_pair("q", q);
    format!("{path}?{}", serializer.finish())
}

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

async fn fetch_paginated_with_query(
    helper: &TestHelper,
    q: &str,
    filters: &TraceFilters,
) -> (StatusCode, Vec<u8>) {
    let body = serde_json::to_string(&filters).unwrap();
    let request = Request::builder()
        .uri(with_q("/scouter/trace/paginated", q))
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();
    let response = helper.send_oneshot(request).await;
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    (status, body)
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

async fn fetch_spans_from_filters_raw(
    helper: &TestHelper,
    uri: &str,
    filters: &TraceFilters,
) -> (StatusCode, Vec<u8>) {
    let body = serde_json::to_string(filters).unwrap();
    let request = Request::builder()
        .uri(uri)
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();
    let response = helper.send_oneshot(request).await;
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    (status, body)
}

async fn fetch_facets(helper: &TestHelper, filters: &TraceFilters) -> TraceFacetsResponse {
    let body = serde_json::to_string(filters).unwrap();
    let request = Request::builder()
        .uri("/scouter/trace/facets")
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
    let (status, body) = fetch_metrics_raw(helper, "/scouter/trace/metrics", request).await;
    assert_eq!(status, StatusCode::OK);
    serde_json::from_slice(&body).unwrap()
}

async fn fetch_metrics_raw(
    helper: &TestHelper,
    uri: &str,
    request: &TraceMetricsRequest,
) -> (StatusCode, Vec<u8>) {
    let req = Request::builder()
        .uri(uri)
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(request).unwrap()))
        .unwrap();
    let response = helper.send_oneshot(req).await;
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    (status, body)
}

async fn fetch_facets_raw(
    helper: &TestHelper,
    uri: &str,
    filters: &TraceFilters,
) -> (StatusCode, Vec<u8>) {
    let body = serde_json::to_string(filters).unwrap();
    let request = Request::builder()
        .uri(uri)
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();
    let response = helper.send_oneshot(request).await;
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    (status, body)
}

async fn wait_for_paginated_count(helper: &TestHelper, expected_count: usize) {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let _ = shutdown_trace_cache(&helper.pool).await.unwrap();
        let page = fetch_paginated(
            helper,
            &TraceFilters {
                limit: Some(200),
                ..Default::default()
            },
        )
        .await;
        if page.items.len() >= expected_count {
            return;
        }

        assert!(
            Instant::now() < deadline,
            "Timed out waiting for {expected_count} traces, saw {}",
            page.items.len()
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn wait_for_genai_trace_spans(
    helper: &TestHelper,
    trace_id: &TraceId,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    expected_count: usize,
) {
    let deadline = Instant::now() + Duration::from_secs(8);
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

    wait_for_paginated_count(&helper, 100).await;

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
        service_namespace: None,
        service_version: None,
        service_instance_id: None,
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
        start_time,
        end_time,
        bucket_interval: "hour".to_string(),
        clause: None,
        entity_uid: None,
        query: None,
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
        service_namespace: None,
        service_version: None,
        service_instance_id: None,
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
    let attr_filters = TraceFilters::parse("component:kafka").unwrap();
    let attr_batch = fetch_paginated(&helper, &attr_filters).await;
    assert!(
        !attr_batch.items.is_empty(),
        "Should return records with attribute filter"
    );
}

#[tokio::test]
async fn test_trace_search_query_user_flow() {
    let helper = setup_test().await;
    helper.generate_trace_data().await.unwrap();
    wait_for_paginated_count(&helper, 100).await;

    let body_filters = TraceFilters {
        limit: Some(200),
        ..TraceFilters::parse("component:kafka").unwrap()
    };
    let body_page = fetch_paginated(&helper, &body_filters).await;
    assert!(
        !body_page.items.is_empty(),
        "component:kafka should match seeded traces"
    );

    let (status, post_body) = fetch_paginated_with_query(
        &helper,
        "component:kafka",
        &TraceFilters {
            limit: Some(200),
            ..Default::default()
        },
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let post_page: TracePaginationResponse = serde_json::from_slice(&post_body).unwrap();
    assert_eq!(
        post_page.items.len(),
        body_page.items.len(),
        "POST URL query should merge into an empty body as the parsed clause"
    );

    let (status, _body) = fetch_paginated_with_query(
        &helper,
        "(start_time:2026-01-01T00:00:00Z component:kafka)",
        &TraceFilters::default(),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "top-level-only fields inside groups must be rejected"
    );
}

#[tokio::test]
async fn test_trace_search_q_and_body_are_and_merged() {
    let helper = setup_test().await;
    helper.generate_trace_data().await.unwrap();
    wait_for_paginated_count(&helper, 100).await;

    let q_only = fetch_paginated(
        &helper,
        &TraceFilters {
            limit: Some(200),
            ..TraceFilters::parse("component:kafka").unwrap()
        },
    )
    .await;
    assert!(
        !q_only.items.is_empty(),
        "component:kafka should match seeded traces"
    );

    let body = TraceFilters {
        clause: Some(FilterClause::Service("__no_such_service__".to_string())),
        limit: Some(200),
        ..Default::default()
    };
    let (status, merged_body) = fetch_paginated_with_query(&helper, "component:kafka", &body).await;
    assert_eq!(status, StatusCode::OK);
    let merged: TracePaginationResponse = serde_json::from_slice(&merged_body).unwrap();

    assert!(
        merged.items.is_empty(),
        "AND-merged query/body filters should return the exact empty intersection"
    );

    let (status, empty_body) =
        fetch_paginated_with_query(&helper, "   ", &TraceFilters::default()).await;
    assert_eq!(status, StatusCode::OK);
    let empty_query: TracePaginationResponse = serde_json::from_slice(&empty_body).unwrap();
    let default_page = fetch_paginated(&helper, &TraceFilters::default()).await;
    assert_eq!(empty_query.items.len(), default_page.items.len());
}

#[tokio::test]
async fn test_trace_query_is_merged_for_metrics_facets_and_spans() {
    let helper = setup_test().await;
    helper.generate_trace_data().await.unwrap();
    wait_for_paginated_count(&helper, 100).await;

    let now = Utc::now();
    let metrics_base = TraceMetricsRequest {
        start_time: now - chrono::Duration::hours(24),
        end_time: now + chrono::Duration::hours(1),
        bucket_interval: "hour".to_string(),
        clause: None,
        entity_uid: None,
        query: None,
    };
    let metrics_body = TraceMetricsRequest {
        clause: TraceFilters::parse("component:kafka").unwrap().clause,
        ..metrics_base.clone()
    };

    let metrics_from_body = fetch_metrics(&helper, &metrics_body).await;
    let metrics_query_body = TraceMetricsRequest {
        query: Some("component:kafka".to_string()),
        ..metrics_base.clone()
    };
    let (status, metrics_q_body) =
        fetch_metrics_raw(&helper, "/scouter/trace/metrics", &metrics_query_body).await;
    assert_eq!(status, StatusCode::OK);
    let metrics_from_q: TraceMetricsResponse = serde_json::from_slice(&metrics_q_body).unwrap();
    assert_eq!(
        metrics_from_q
            .metrics
            .iter()
            .map(|bucket| bucket.trace_count)
            .sum::<i64>(),
        metrics_from_body
            .metrics
            .iter()
            .map(|bucket| bucket.trace_count)
            .sum::<i64>()
    );

    let metrics_and_body = TraceMetricsRequest {
        clause: Some(FilterClause::Service("__no_such_service__".to_string())),
        query: Some("component:kafka".to_string()),
        ..metrics_base
    };
    let (status, metrics_merged_body) =
        fetch_metrics_raw(&helper, "/scouter/trace/metrics", &metrics_and_body).await;
    assert_eq!(status, StatusCode::OK);
    let metrics_merged: TraceMetricsResponse =
        serde_json::from_slice(&metrics_merged_body).unwrap();
    assert_eq!(
        metrics_merged
            .metrics
            .iter()
            .map(|bucket| bucket.trace_count)
            .sum::<i64>(),
        0,
        "metrics query/body merge should return the exact empty intersection"
    );

    let body_filters = TraceFilters::parse("component:kafka").unwrap();
    let body_facets = fetch_facets(&helper, &body_filters).await;
    let (status, facets_q_body) = fetch_facets_raw(
        &helper,
        &with_q("/scouter/trace/facets", "component:kafka"),
        &TraceFilters::default(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let facets_from_q: TraceFacetsResponse = serde_json::from_slice(&facets_q_body).unwrap();
    assert_eq!(facets_from_q.total_count, body_facets.total_count);

    let merged_filters = TraceFilters {
        clause: Some(FilterClause::Service("__no_such_service__".to_string())),
        ..Default::default()
    };
    let (status, merged_facets_body) = fetch_facets_raw(
        &helper,
        &with_q("/scouter/trace/facets", "component:kafka"),
        &merged_filters,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let merged_facets: TraceFacetsResponse = serde_json::from_slice(&merged_facets_body).unwrap();
    assert_eq!(
        merged_facets.total_count, 0,
        "facets query/body merge should return the exact empty intersection"
    );

    let body_spans = fetch_spans_from_filters(&helper, &body_filters).await;
    let (status, spans_q_body) = fetch_spans_from_filters_raw(
        &helper,
        &with_q("/scouter/trace/spans/filters", "component:kafka"),
        &TraceFilters::default(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let spans_from_q: TraceSpansResponse = serde_json::from_slice(&spans_q_body).unwrap();
    assert_eq!(spans_from_q.spans.len(), body_spans.spans.len());

    let (status, merged_spans_body) = fetch_spans_from_filters_raw(
        &helper,
        &with_q("/scouter/trace/spans/filters", "component:kafka"),
        &merged_filters,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let merged_spans: TraceSpansResponse = serde_json::from_slice(&merged_spans_body).unwrap();
    assert!(
        merged_spans.spans.is_empty(),
        "spans query/body merge should return the exact empty intersection"
    );

    let invalid_q = "(start_time:2026-01-01T00:00:00Z component:kafka)";
    let (status, _) = fetch_metrics_raw(
        &helper,
        "/scouter/trace/metrics",
        &TraceMetricsRequest {
            start_time: now - chrono::Duration::hours(24),
            end_time: now + chrono::Duration::hours(1),
            bucket_interval: "hour".to_string(),
            clause: None,
            entity_uid: None,
            query: Some(invalid_q.to_string()),
        },
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = fetch_facets_raw(
        &helper,
        &with_q("/scouter/trace/facets", invalid_q),
        &TraceFilters::default(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = fetch_spans_from_filters_raw(
        &helper,
        &with_q("/scouter/trace/spans/filters", invalid_q),
        &TraceFilters::default(),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_spans_from_filters_accepts_mixed_or_clause() {
    let helper = setup_test().await;
    helper.generate_trace_data().await.unwrap();
    wait_for_paginated_count(&helper, 100).await;

    let all = fetch_paginated(
        &helper,
        &TraceFilters {
            limit: Some(200),
            ..Default::default()
        },
    )
    .await;
    let service_name = all
        .items
        .first()
        .expect("generated traces should include at least one summary")
        .service_name
        .clone();

    let filters = TraceFilters {
        clause: Some(FilterClause::Or(vec![
            FilterClause::Service(service_name),
            FilterClause::Attr {
                key: "component".to_string(),
                value: "kafka".to_string(),
            },
        ])),
        limit: Some(25),
        ..Default::default()
    };
    let (status, body) =
        fetch_spans_from_filters_raw(&helper, "/scouter/trace/spans/filters", &filters).await;

    // This route uses the fourth planner consumer; mixed OR must remain a legal
    // query shape instead of failing during summary/span planning.
    assert_eq!(
        status,
        StatusCode::OK,
        "unexpected response: {}",
        String::from_utf8_lossy(&body)
    );
    let spans: TraceSpansResponse = serde_json::from_slice(&body).unwrap();
    assert!(!spans.spans.is_empty());
}

#[tokio::test]
async fn test_trace_pagination() {
    let helper = setup_test().await;
    helper.generate_trace_data().await.unwrap();

    wait_for_paginated_count(&helper, 100).await;

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
async fn test_post_trace_search_cursor_and_validation_paths() {
    let helper = setup_test().await;
    helper.generate_trace_data().await.unwrap();
    wait_for_paginated_count(&helper, 100).await;

    let first_filters = TraceFilters {
        limit: Some(2),
        ..Default::default()
    };
    let (status, body) =
        fetch_paginated_with_query(&helper, "component:kafka", &first_filters).await;
    assert_eq!(status, StatusCode::OK);
    let first_page: TracePaginationResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(first_page.items.len(), 2);
    assert!(first_page.has_next);

    let next_cursor = first_page.next_cursor.as_ref().unwrap();
    let second_filters = TraceFilters {
        limit: Some(2),
        cursor_start_time: Some(next_cursor.start_time),
        cursor_trace_id: Some(next_cursor.trace_id.clone()),
        direction: Some("next".to_string()),
        ..Default::default()
    };
    let (status, body) =
        fetch_paginated_with_query(&helper, "component:kafka", &second_filters).await;
    assert_eq!(status, StatusCode::OK);
    let second_page: TracePaginationResponse = serde_json::from_slice(&body).unwrap();
    assert!(!second_page.items.is_empty());
    assert!(second_page.has_previous);

    let first_ids: HashSet<String> = first_page
        .items
        .iter()
        .map(|item| item.trace_id.clone())
        .collect();
    let second_ids: HashSet<String> = second_page
        .items
        .iter()
        .map(|item| item.trace_id.clone())
        .collect();
    assert!(first_ids.is_disjoint(&second_ids));

    let mut previous_start = first_page.items[0].start_time;
    for item in first_page
        .items
        .iter()
        .skip(1)
        .chain(second_page.items.iter())
    {
        assert!(item.start_time <= previous_start);
        previous_start = item.start_time;
    }

    let (status, _) =
        fetch_paginated_with_query(&helper, "trace_id:not-a-hex-id", &TraceFilters::default())
            .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = fetch_paginated_with_query(
        &helper,
        "component:kafka",
        &TraceFilters {
            trace_ids: Some(vec!["not-a-hex-id".to_string()]),
            ..Default::default()
        },
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = fetch_paginated_with_query(
        &helper,
        "component:kafka",
        &TraceFilters {
            cursor_start_time: Some(next_cursor.start_time),
            cursor_trace_id: Some("not-a-hex-id".to_string()),
            direction: Some("next".to_string()),
            ..Default::default()
        },
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = fetch_paginated_with_query(
        &helper,
        "component:kafka",
        &TraceFilters {
            direction: Some("sideways".to_string()),
            ..Default::default()
        },
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_paginated_traces_duration_min() {
    let helper = setup_test().await;
    helper
        .generate_traces_with_durations(&[50, 200, 800, 1500])
        .await
        .unwrap();

    wait_for_paginated_count(&helper, 4).await;

    let filters = TraceFilters {
        clause: Some(FilterClause::DurationMinMs(500)),
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
    wait_for_paginated_count(&helper, 4).await;

    let filters = TraceFilters {
        clause: Some(FilterClause::DurationMaxMs(300)),
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
    wait_for_paginated_count(&helper, 5).await;

    let filters = TraceFilters {
        clause: Some(FilterClause::And(vec![
            FilterClause::DurationMinMs(100),
            FilterClause::DurationMaxMs(500),
        ])),
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
    wait_for_paginated_count(&helper, 4).await;

    let now = Utc::now();
    let unfiltered = fetch_metrics(
        &helper,
        &TraceMetricsRequest {
            start_time: now - chrono::Duration::hours(2),
            end_time: now + chrono::Duration::hours(1),
            bucket_interval: "hour".to_string(),
            clause: None,
            entity_uid: None,
            query: None,
        },
    )
    .await;
    let unfiltered_total: i64 = unfiltered.metrics.iter().map(|m| m.trace_count).sum();

    let min_only = fetch_metrics(
        &helper,
        &TraceMetricsRequest {
            start_time: now - chrono::Duration::hours(2),
            end_time: now + chrono::Duration::hours(1),
            bucket_interval: "hour".to_string(),
            clause: Some(FilterClause::DurationMinMs(500)),
            entity_uid: None,
            query: None,
        },
    )
    .await;
    let min_only_total: i64 = min_only.metrics.iter().map(|m| m.trace_count).sum();

    assert!(
        min_only_total < unfiltered_total,
        "Duration filter should shrink count"
    );
    assert_eq!(min_only_total, 2);
    for bucket in &min_only.metrics {
        assert!(bucket.avg_duration_ms >= 500.0);
    }

    let max_only = fetch_metrics(
        &helper,
        &TraceMetricsRequest {
            start_time: now - chrono::Duration::hours(2),
            end_time: now + chrono::Duration::hours(1),
            bucket_interval: "hour".to_string(),
            clause: Some(FilterClause::DurationMaxMs(300)),
            entity_uid: None,
            query: None,
        },
    )
    .await;
    let max_only_total: i64 = max_only.metrics.iter().map(|m| m.trace_count).sum();
    assert_eq!(max_only_total, 2);
    for bucket in &max_only.metrics {
        assert!(bucket.avg_duration_ms <= 300.0);
    }

    let bounded = fetch_metrics(
        &helper,
        &TraceMetricsRequest {
            start_time: now - chrono::Duration::hours(2),
            end_time: now + chrono::Duration::hours(1),
            bucket_interval: "hour".to_string(),
            clause: Some(FilterClause::And(vec![
                FilterClause::DurationMinMs(200),
                FilterClause::DurationMaxMs(800),
            ])),
            entity_uid: None,
            query: None,
        },
    )
    .await;
    let bounded_total: i64 = bounded.metrics.iter().map(|m| m.trace_count).sum();
    assert_eq!(bounded_total, 2);
    for bucket in &bounded.metrics {
        assert!((200.0..=800.0).contains(&bucket.avg_duration_ms));
    }

    let wider_max = fetch_metrics(
        &helper,
        &TraceMetricsRequest {
            start_time: now - chrono::Duration::hours(2),
            end_time: now + chrono::Duration::hours(1),
            bucket_interval: "hour".to_string(),
            clause: Some(FilterClause::DurationMaxMs(1_000)),
            entity_uid: None,
            query: None,
        },
    )
    .await;
    let wider_max_total: i64 = wider_max.metrics.iter().map(|m| m.trace_count).sum();
    assert_eq!(wider_max_total, 3);
    assert!(
        wider_max_total > max_only_total,
        "Different duration_max values must not share cached results"
    );
}

#[tokio::test]
async fn test_spans_from_filters_duration() {
    let helper = setup_test().await;
    helper
        .generate_traces_with_durations(&[100, 1500])
        .await
        .unwrap();
    wait_for_paginated_count(&helper, 2).await;

    let min_only_filters = TraceFilters {
        clause: Some(FilterClause::DurationMinMs(1000)),
        ..Default::default()
    };
    let min_only_response = fetch_spans_from_filters(&helper, &min_only_filters).await;

    assert!(
        !min_only_response.spans.is_empty(),
        "Expected spans for the slow trace"
    );
    let min_only_trace_ids: HashSet<_> = min_only_response
        .spans
        .iter()
        .map(|s| s.trace_id.clone())
        .collect();
    assert_eq!(
        min_only_trace_ids.len(),
        1,
        "spans-from-filters returns one trace at a time"
    );

    let max_only_response = fetch_spans_from_filters(
        &helper,
        &TraceFilters {
            clause: Some(FilterClause::DurationMaxMs(200)),
            ..Default::default()
        },
    )
    .await;
    assert!(!max_only_response.spans.is_empty());
    let max_only_trace_ids: HashSet<_> = max_only_response
        .spans
        .iter()
        .map(|s| s.trace_id.clone())
        .collect();
    assert_eq!(max_only_trace_ids.len(), 1);

    let bounded_response = fetch_spans_from_filters(
        &helper,
        &TraceFilters {
            clause: Some(FilterClause::And(vec![
                FilterClause::DurationMinMs(1200),
                FilterClause::DurationMaxMs(2000),
            ])),
            ..Default::default()
        },
    )
    .await;
    assert!(!bounded_response.spans.is_empty());
    let bounded_trace_ids: HashSet<_> = bounded_response
        .spans
        .iter()
        .map(|s| s.trace_id.clone())
        .collect();
    assert_eq!(bounded_trace_ids.len(), 1);
    assert_ne!(
        max_only_trace_ids, bounded_trace_ids,
        "Max-only and bounded-range filters should select different traces"
    );
}

#[tokio::test]
async fn test_paginated_traces_inverted_range_rejected() {
    let helper = setup_test().await;
    helper
        .generate_traces_with_durations(&[100, 200, 300])
        .await
        .unwrap();
    wait_for_paginated_count(&helper, 3).await;

    let body = serde_json::to_string(&TraceFilters {
        clause: Some(FilterClause::And(vec![
            FilterClause::DurationMinMs(500),
            FilterClause::DurationMaxMs(100),
        ])),
        ..Default::default()
    })
    .unwrap();
    let request = Request::builder()
        .uri("/scouter/trace/paginated")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();
    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
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
    wait_for_genai_trace_spans(
        &helper,
        &trace_id,
        now - chrono::Duration::hours(1),
        now + chrono::Duration::hours(1),
        1,
    )
    .await;

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
async fn test_genai_trace_metrics_route_sensitive_content_auth() {
    let helper = setup_test().await;
    let trace_id = TraceId::from_bytes([58u8; 16]);
    let now = Utc::now();
    helper
        .genai_service
        .write_records(vec![build_genai_span(trace_id, 1, now)])
        .await
        .unwrap();
    wait_for_genai_trace_spans(
        &helper,
        &trace_id,
        now - chrono::Duration::hours(1),
        now + chrono::Duration::hours(1),
        1,
    )
    .await;

    let limited_username = format!("trace_limited_{}", now.timestamp_nanos_opt().unwrap_or(0));
    let limited_password = "trace_limited_pw";
    let create_req = CreateUserRequest {
        username: limited_username.clone(),
        password: limited_password.to_string(),
        email: format!("{limited_username}@example.com"),
        permissions: Some(vec!["read:space".to_string()]),
        group_permissions: Some(vec!["user".to_string()]),
        role: Some("user".to_string()),
        active: Some(true),
    };
    let create_user_request = Request::builder()
        .uri("/scouter/user")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&create_req).unwrap()))
        .unwrap();
    let create_user_response = helper.send_oneshot(create_user_request).await;
    assert_eq!(create_user_response.status(), StatusCode::OK);

    let limited_token = helper
        .login_with_credentials(&limited_username, limited_password)
        .await;
    let sensitive_body = serde_json::json!({
        "start_time": (now - chrono::Duration::hours(1)).to_rfc3339(),
        "end_time": (now + chrono::Duration::hours(1)).to_rfc3339(),
        "bucket_interval": "hour",
        "model_pricing": {},
        "span_limit": 10,
        "include_sensitive_content": true
    });

    let limited_request = Request::builder()
        .uri(format!(
            "/scouter/genai/traces/{}/metrics",
            trace_id.to_hex()
        ))
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&sensitive_body).unwrap()))
        .unwrap();
    let limited_response = helper
        .send_oneshot_with_token(limited_request, &limited_token.token)
        .await;
    assert_eq!(limited_response.status(), StatusCode::FORBIDDEN);

    let admin_request = Request::builder()
        .uri(format!(
            "/scouter/genai/traces/{}/metrics",
            trace_id.to_hex()
        ))
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&sensitive_body).unwrap()))
        .unwrap();
    let admin_response = helper.send_oneshot(admin_request).await;
    assert_eq!(admin_response.status(), StatusCode::OK);
    let body = admin_response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(response_json["sensitive_content_redacted"], false);
    let spans = response_json["spans"].as_array().unwrap();
    assert_eq!(spans.len(), 1);
    assert!(spans[0]["input_messages"].is_string());
    assert!(spans[0]["output_messages"].is_string());
    assert!(spans[0]["system_instructions"].is_string());
    assert!(spans[0]["tool_definitions"].is_string());
}

#[tokio::test]
async fn test_genai_trace_metrics_route_span_limit_only_truncates_payload() {
    let helper = setup_test().await;
    let trace_id = TraceId::from_bytes([59u8; 16]);
    let now = Utc::now();
    let records: Vec<GenAiSpanRecord> = (0..6)
        .map(|i| build_genai_span(trace_id, (i + 1) as u8, now + chrono::Duration::seconds(i)))
        .collect();
    helper.genai_service.write_records(records).await.unwrap();
    wait_for_genai_trace_spans(
        &helper,
        &trace_id,
        now - chrono::Duration::hours(1),
        now + chrono::Duration::hours(1),
        6,
    )
    .await;

    let request = Request::builder()
        .uri(format!(
            "/scouter/genai/traces/{}/metrics",
            trace_id.to_hex()
        ))
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "start_time": (now - chrono::Duration::hours(1)).to_rfc3339(),
                "end_time": (now + chrono::Duration::hours(1)).to_rfc3339(),
                "bucket_interval": "hour",
                "span_limit": 3,
                "include_sensitive_content": false
            })
            .to_string(),
        ))
        .unwrap();
    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let response_json: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(response_json["span_limit"], 3);
    assert_eq!(response_json["spans_truncated"], true);
    assert_eq!(response_json["spans"].as_array().unwrap().len(), 3);
    assert_eq!(
        response_json["operation_breakdown"]["operations"][0]["span_count"],
        6
    );
    assert_eq!(
        response_json["token_metrics"]["buckets"][0]["total_input_tokens"],
        72
    );
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

    let now = Utc::now();
    let invalid_time_bounds_request = Request::builder()
        .uri(format!("/scouter/genai/traces/{valid_trace_id}/metrics"))
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "bucket_interval": "hour",
                "start_time": now.to_rfc3339(),
                "end_time": now.to_rfc3339(),
            })
            .to_string(),
        ))
        .unwrap();
    let invalid_time_bounds_response = helper.send_oneshot(invalid_time_bounds_request).await;
    assert_eq!(
        invalid_time_bounds_response.status(),
        StatusCode::BAD_REQUEST
    );

    let invalid_window_request = Request::builder()
        .uri(format!("/scouter/genai/traces/{valid_trace_id}/metrics"))
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "bucket_interval": "hour",
                "start_time": (now - chrono::Duration::days(31)).to_rfc3339(),
                "end_time": now.to_rfc3339(),
            })
            .to_string(),
        ))
        .unwrap();
    let invalid_window_response = helper.send_oneshot(invalid_window_request).await;
    assert_eq!(invalid_window_response.status(), StatusCode::BAD_REQUEST);

    let span_limit_clamp_request = Request::builder()
        .uri(format!("/scouter/genai/traces/{valid_trace_id}/metrics"))
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({
                "bucket_interval": "hour",
                "span_limit": 999_999,
            })
            .to_string(),
        ))
        .unwrap();
    let span_limit_clamp_response = helper.send_oneshot(span_limit_clamp_request).await;
    assert_eq!(span_limit_clamp_response.status(), StatusCode::OK);
    let clamp_body = span_limit_clamp_response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let clamp_json: serde_json::Value = serde_json::from_slice(&clamp_body).unwrap();
    assert_eq!(clamp_json["span_limit"], 5_000);
}

#[tokio::test]
async fn test_trace_facets() {
    let helper = setup_test().await;
    helper.generate_trace_data().await.unwrap();

    wait_for_paginated_count(&helper, 100).await;

    // Unfiltered: expect 100 traces total, services sum to 100, status_codes non-empty
    let facets = fetch_facets(&helper, &TraceFilters::default()).await;
    assert_eq!(facets.total_count, 100, "expected 100 total traces");
    assert!(
        !facets.services.is_empty(),
        "should have at least one service"
    );
    let service_sum: i64 = facets.services.iter().map(|d| d.trace_count).sum();
    assert_eq!(service_sum, 100, "service counts should sum to total_count");
    assert!(
        !facets.status_codes.is_empty(),
        "should have at least one status_code bucket"
    );
    assert!(
        facets
            .status_codes
            .iter()
            .any(|d| d.value == "UNSET" || d.value == "OK" || d.value == "ERROR"),
        "status_code buckets should contain known labels"
    );

    // Service filter: single service → services.len() == 1, consistent count
    let first_svc = facets.services[0].value.clone();
    let svc_facets = fetch_facets(
        &helper,
        &TraceFilters {
            clause: Some(FilterClause::Service(first_svc)),
            ..Default::default()
        },
    )
    .await;
    assert_eq!(svc_facets.services.len(), 1, "filtered to one service");
    assert_eq!(
        svc_facets.services[0].trace_count, svc_facets.total_count,
        "single-service count == total_count"
    );

    // Duration filter: narrow range should reduce total
    let duration_facets = fetch_facets(
        &helper,
        &TraceFilters {
            clause: Some(FilterClause::DurationMinMs(500)),
            ..Default::default()
        },
    )
    .await;
    assert!(
        duration_facets.total_count <= 100,
        "duration filter should not exceed 100"
    );

    // has_errors = Some(true) — only error traces
    let error_facets = fetch_facets(
        &helper,
        &TraceFilters {
            clause: Some(FilterClause::HasErrors(true)),
            ..Default::default()
        },
    )
    .await;
    assert!(
        error_facets.total_count > 0,
        "expected at least one error trace (5% error rate in mock data)"
    );
    assert!(
        error_facets
            .status_codes
            .iter()
            .all(|d| d.value == "ERROR" || d.value == "UNSET"),
        "error filter should only return ERROR or UNSET status codes"
    );

    // has_errors = Some(false) — no error traces
    let no_error_facets = fetch_facets(
        &helper,
        &TraceFilters {
            clause: Some(FilterClause::HasErrors(false)),
            ..Default::default()
        },
    )
    .await;
    assert!(
        no_error_facets
            .status_codes
            .iter()
            .all(|d| d.value != "ERROR"),
        "no-error filter should not return ERROR bucket"
    );

    // component:kafka matches ~10% of mock spans
    let attr_facets = fetch_facets(&helper, &TraceFilters::parse("component:kafka").unwrap()).await;
    assert!(
        attr_facets.total_count < facets.total_count,
        "attribute filter should reduce total_count below unfiltered"
    );
    assert!(
        attr_facets.total_count > 0,
        "component=kafka should match at least one trace"
    );

    // nonexistent attribute value → lit(false) branch → zero results
    let empty_attr_facets = fetch_facets(
        &helper,
        &TraceFilters::parse("component:nonexistent_xyz_abc").unwrap(),
    )
    .await;
    assert_eq!(
        empty_attr_facets.total_count, 0,
        "nonexistent attribute filter should return 0 traces"
    );
    assert!(
        empty_attr_facets.services.is_empty(),
        "nonexistent attribute filter should return empty services"
    );
    assert!(
        empty_attr_facets.status_codes.is_empty(),
        "nonexistent attribute filter should return empty status_codes"
    );
}

// Regression: clause + time bounds must not fail with "column not found"
// for PARTITION_DATE_COL. Previously, select_columns was called before the partition
// filter, stripping the column from the schema.
#[tokio::test]
async fn test_paginated_traces_attribute_filter_with_time_bounds() {
    let helper = setup_test().await;
    helper.generate_trace_data().await.unwrap();
    wait_for_paginated_count(&helper, 100).await;

    let now = Utc::now();
    let start = now - chrono::Duration::hours(2);
    let end = now + chrono::Duration::hours(1);

    // clause + start_time + end_time: the combination that triggered the bug
    let result = fetch_paginated(
        &helper,
        &TraceFilters {
            clause: TraceFilters::parse("component:kafka").unwrap().clause,
            start_time: Some(start),
            end_time: Some(end),
            ..Default::default()
        },
    )
    .await;

    // Result may be empty (depends on mock data), but the query must not panic/error
    let _ = result;

    // Verify with a time window that should include all data
    let wide_result = fetch_paginated(
        &helper,
        &TraceFilters {
            clause: TraceFilters::parse("component:kafka").unwrap().clause,
            start_time: Some(now - chrono::Duration::hours(24)),
            end_time: Some(now + chrono::Duration::hours(1)),
            ..Default::default()
        },
    )
    .await;
    assert!(
        !wide_result.items.is_empty(),
        "attribute + wide time window must return kafka-tagged traces"
    );
}

// Regression: same bug in get_trace_facets clause path.
#[tokio::test]
async fn test_trace_facets_attribute_filter_with_time_bounds() {
    let helper = setup_test().await;
    helper.generate_trace_data().await.unwrap();
    wait_for_paginated_count(&helper, 100).await;

    let now = Utc::now();

    // Narrow window: query must not error — result may legitimately be empty
    let narrow = fetch_facets(
        &helper,
        &TraceFilters {
            clause: TraceFilters::parse("component:kafka").unwrap().clause,
            start_time: Some(now - chrono::Duration::hours(1)),
            end_time: Some(now + chrono::Duration::minutes(5)),
            ..Default::default()
        },
    )
    .await;
    let _ = narrow;

    // Wide window: must find kafka-tagged traces
    let wide = fetch_facets(
        &helper,
        &TraceFilters {
            clause: TraceFilters::parse("component:kafka").unwrap().clause,
            start_time: Some(now - chrono::Duration::hours(24)),
            end_time: Some(now + chrono::Duration::hours(1)),
            ..Default::default()
        },
    )
    .await;
    assert!(
        wide.total_count > 0,
        "attribute + wide time window must return kafka-tagged traces in facets"
    );
}

#[tokio::test]
async fn test_trace_facets_empty_store() {
    let helper = setup_test().await;

    let facets = fetch_facets(&helper, &TraceFilters::default()).await;
    assert_eq!(
        facets.total_count, 0,
        "empty store should have total_count 0"
    );
    assert!(
        facets.services.is_empty(),
        "empty store should have no services"
    );
    assert!(
        facets.status_codes.is_empty(),
        "empty store should have no status_codes"
    );
}

#[tokio::test]
async fn test_genai_projection_null_source_fields() {
    let helper = setup_test().await;
    let trace_id = TraceId::from_bytes([70u8; 16]);
    let now = Utc::now();

    // Span with some sensitive fields genuinely None in source
    let sparse_span = GenAiSpanRecord {
        trace_id,
        span_id: SpanId::from_bytes([1u8; 8]),
        service_name: "genai-service".to_string(),
        start_time: now,
        end_time: Some(now + chrono::Duration::milliseconds(10)),
        duration_ms: 10,
        status_code: 0,
        input_messages: Some(r#"[{"role":"user","content":"hello"}]"#.to_string()),
        output_messages: Some(r#"[{"role":"assistant","content":"hi"}]"#.to_string()),
        system_instructions: None,
        tool_definitions: None,
        ..Default::default()
    };

    helper
        .genai_service
        .write_records(vec![sparse_span])
        .await
        .unwrap();
    wait_for_genai_trace_spans(
        &helper,
        &trace_id,
        now - chrono::Duration::hours(1),
        now + chrono::Duration::hours(1),
        1,
    )
    .await;

    // Fetch with include_sensitive_content=true (admin token)
    let body = serde_json::json!({
        "start_time": (now - chrono::Duration::hours(1)).to_rfc3339(),
        "end_time": (now + chrono::Duration::hours(1)).to_rfc3339(),
        "bucket_interval": "hour",
        "span_limit": 10,
        "include_sensitive_content": true
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
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let response_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    let spans = response_json["spans"].as_array().unwrap();
    assert_eq!(spans.len(), 1);
    // Non-null source fields are present
    assert!(
        spans[0]["input_messages"].is_string(),
        "input_messages should be non-null"
    );
    assert!(
        spans[0]["output_messages"].is_string(),
        "output_messages should be non-null"
    );
    // Null source fields remain null (not redacted-null vs source-null confusion)
    assert!(
        spans[0]["system_instructions"].is_null(),
        "system_instructions was None in source, should be null even with sensitive content"
    );
    assert!(
        spans[0]["tool_definitions"].is_null(),
        "tool_definitions was None in source, should be null even with sensitive content"
    );
}
