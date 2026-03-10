use crate::common::setup_test;
use crate::common::TestHelper;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use http_body_util::BodyExt;
use scouter_sql::sql::aggregator::shutdown_trace_cache;
use scouter_types::{
    sql::TraceFilters, SpansFromTagsRequest, TraceMetricsRequest, TraceMetricsResponse,
    TracePaginationResponse, TraceRequest, TraceSpansResponse,
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
