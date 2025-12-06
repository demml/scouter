use crate::common::setup_test;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use http_body_util::BodyExt;
use scouter_types::{
    sql::TraceFilters, TraceMetricsRequest, TraceMetricsResponse, TracePaginationResponse,
    TraceRequest, TraceSpansResponse,
};

#[tokio::test]
async fn test_tracing() {
    let helper = setup_test().await;
    helper.generate_trace_data().await.unwrap();

    // get paginated traces
    let mut filters = TraceFilters::default();
    let body = serde_json::to_string(&filters).unwrap();

    let request = Request::builder()
        .uri("/scouter/trace/paginated")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = helper.send_oneshot(request).await;

    //assert response
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();

    let first_batch: TracePaginationResponse = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        first_batch.items.len(),
        50,
        "First batch should have 50 records"
    );

    let last_record_cursor = &first_batch.next_cursor.unwrap();
    filters = filters.next_page(last_record_cursor);

    let body = serde_json::to_string(&filters).unwrap();
    let request = Request::builder()
        .uri("/scouter/trace/paginated")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let second_batch: TracePaginationResponse = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        second_batch.items.len(),
        50,
        "Next batch should have 50 records"
    );

    let next_first_record = second_batch.items.first().unwrap();
    assert!(
        next_first_record.created_at <= last_record_cursor.created_at,
        "Next batch first record timestamp is not less than or equal to last record timestamp"
    );

    let filtered_record = first_batch
        .items
        .iter()
        .find(|record| record.span_count > Some(5))
        .unwrap();

    filters.cursor_created_at = None;
    filters.cursor_trace_id = None;
    filters.service_name = Some(filtered_record.service_name.clone());

    let body = serde_json::to_string(&filters).unwrap();
    let request = Request::builder()
        .uri("/scouter/trace/paginated")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let filtered_batch: TracePaginationResponse = serde_json::from_slice(&body).unwrap();

    assert!(
        !filtered_batch.items.is_empty(),
        "Should return records with specified filters"
    );

    // now get spans for one of the traces
    let params = TraceRequest {
        trace_id: filtered_batch.items.first().unwrap().trace_id.clone(),
        service_name: None,
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

    let start_time = filtered_record.created_at - chrono::Duration::hours(24);
    let end_time = filtered_record.created_at + chrono::Duration::minutes(5);

    // make request for trace metrics
    let metrics_request = TraceMetricsRequest {
        service_name: None,
        start_time,
        end_time,
        bucket_interval: "60 minutes".to_string(),
    };

    let query_string = serde_qs::to_string(&metrics_request).unwrap();
    let request = Request::builder()
        .uri(format!("/scouter/trace/metrics?{query_string}"))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    // assert we have data points
    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let metrics_response: TraceMetricsResponse = serde_json::from_slice(&body).unwrap();

    assert!(metrics_response.metrics.len() >= 10);
}
