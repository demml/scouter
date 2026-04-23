use crate::common::{setup_test, NAME, SPACE, VERSION};
use std::time::Duration;

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
use scouter_drift::psi::PsiMonitor;
use scouter_sql::sql::aggregator::shutdown_trace_cache;
use scouter_types::contracts::DriftRequest;
use scouter_types::custom::{
    CustomDriftProfile, CustomMetric, CustomMetricAlertConfig, CustomMetricDriftConfig,
};
use scouter_types::psi::BinnedPsiFeatureMetrics;
use scouter_types::psi::{PsiAlertConfig, PsiDriftConfig};
use scouter_types::spc::SpcDriftFeatures;
use scouter_types::{
    sql::TraceFilters, AlertThreshold, BinnedMetrics, MessageRecord, TagRecord, TimeInterval,
    TracePaginationResponse, TraceServerRecord,
};
use scouter_types::{TagsRequest, TagsResponse};
use tokio::time::sleep;

#[tokio::test]
async fn test_grpc_insert_spc_records() {
    let helper = setup_test().await;

    // Create a gRPC client
    let client = helper.create_grpc_client().await;

    // Create a drift profile
    let profile = helper.create_drift_profile().await;

    // Generate test records
    let records = helper.get_spc_drift_records(None, &profile.config.uid);

    // Send the request
    let response = client
        .insert_message(serde_json::to_vec(&records).unwrap())
        .await;

    // Assert the response is successful
    assert!(response.is_ok());

    // Sleep to allow processing
    sleep(Duration::from_secs(2)).await;

    // Verify records were inserted by querying via HTTP
    let params = DriftRequest {
        space: SPACE.to_string(),
        uid: profile.config.uid.clone(),
        time_interval: TimeInterval::FifteenMinutes,
        max_data_points: 100,
        ..Default::default()
    };

    let query_string = serde_qs::to_string(&params).unwrap();

    let request = Request::builder()
        .uri(format!("/scouter/drift/spc?{query_string}"))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let results: SpcDriftFeatures = serde_json::from_slice(&body).unwrap();

    assert_eq!(results.features.len(), 10);
}

#[tokio::test]
async fn test_grpc_insert_psi_records() {
    let helper = setup_test().await;

    let client = helper.create_grpc_client().await;

    // Setup PSI profile
    let (array, features) = helper.get_data();
    let alert_config = PsiAlertConfig {
        features_to_monitor: vec![
            "feature_1".to_string(),
            "feature_2".to_string(),
            "feature_3".to_string(),
        ],
        ..Default::default()
    };

    let config = PsiDriftConfig {
        space: SPACE.to_string(),
        name: NAME.to_string(),
        version: VERSION.to_string(),
        alert_config,
        ..Default::default()
    };

    let monitor = PsiMonitor::new();
    let profile = monitor
        .create_2d_drift_profile(&features, &array.view(), &config)
        .unwrap();

    let uid = helper
        .register_drift_profile(profile.create_profile_request().unwrap())
        .await;

    // Generate and send records via gRPC
    let records = helper.get_psi_drift_records(None, &uid);
    let response = client
        .insert_message(serde_json::to_vec(&records).unwrap())
        .await;

    assert!(response.is_ok());

    sleep(Duration::from_secs(2)).await;

    // Verify via HTTP query
    let params = DriftRequest {
        space: SPACE.to_string(),
        uid: uid.clone(),
        time_interval: TimeInterval::FifteenMinutes,
        max_data_points: 100,
        ..Default::default()
    };

    let query_string = serde_qs::to_string(&params).unwrap();

    let request = Request::builder()
        .uri(format!("/scouter/drift/psi?{query_string}"))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let val = response.into_body().collect().await.unwrap().to_bytes();
    let results: BinnedPsiFeatureMetrics = serde_json::from_slice(&val).unwrap();

    assert!(!results.features.is_empty());
}

#[tokio::test]
async fn test_grpc_insert_custom_records() {
    let helper = setup_test().await;

    let client = helper.create_grpc_client().await;

    // Setup custom profile
    let alert_config = CustomMetricAlertConfig::default();
    let config =
        CustomMetricDriftConfig::new(SPACE, NAME, VERSION, 25, alert_config, None).unwrap();

    let alert_threshold = AlertThreshold::Above;
    let metric1 = CustomMetric::new("metric1", 1.0, alert_threshold.clone(), None).unwrap();
    let metric2 = CustomMetric::new("metric2", 1.0, alert_threshold, None).unwrap();
    let profile = CustomDriftProfile::new(config, vec![metric1, metric2]).unwrap();

    let uid = helper
        .register_drift_profile(profile.create_profile_request().unwrap())
        .await;

    // Generate and send records via gRPC
    let records = helper.get_custom_drift_records(None, &uid);
    let response = client
        .insert_message(serde_json::to_vec(&records).unwrap())
        .await;

    assert!(response.is_ok());

    sleep(Duration::from_secs(2)).await;

    // Verify via HTTP query
    let params = DriftRequest {
        space: SPACE.to_string(),
        uid: uid.clone(),
        time_interval: TimeInterval::FifteenMinutes,
        max_data_points: 100,
        ..Default::default()
    };

    let query_string = serde_qs::to_string(&params).unwrap();

    let request = Request::builder()
        .uri(format!("/scouter/drift/custom?{query_string}"))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let val = response.into_body().collect().await.unwrap().to_bytes();
    let results: BinnedMetrics = serde_json::from_slice(&val).unwrap();

    assert!(!results.metrics.is_empty());
}

fn make_grpc_trace_record() -> MessageRecord {
    let trace_id: Vec<u8> = (0u8..16).collect();
    let span_id: Vec<u8> = (0u8..8).collect();

    let service_name_kv = KeyValue {
        key: "service.name".to_string(),
        value: Some(AnyValue {
            value: Some(any_value::Value::StringValue(
                "grpc-test-service".to_string(),
            )),
        }),
    };

    let span = Span {
        trace_id,
        span_id,
        name: "grpc-test-span".to_string(),
        start_time_unix_nano: 1_000_000_000,
        end_time_unix_nano: 2_000_000_000,
        ..Default::default()
    };

    let resource_spans = ResourceSpans {
        resource: Some(Resource {
            attributes: vec![service_name_kv],
            ..Default::default()
        }),
        scope_spans: vec![ScopeSpans {
            spans: vec![span],
            ..Default::default()
        }],
        ..Default::default()
    };

    MessageRecord::TraceServerRecord(TraceServerRecord {
        request: ExportTraceServiceRequest {
            resource_spans: vec![resource_spans],
        },
    })
}

#[tokio::test]
async fn test_grpc_insert_trace_record() {
    let helper = setup_test().await;
    let client = helper.create_grpc_client().await;

    let record = make_grpc_trace_record();
    let response = client
        .insert_message(serde_json::to_vec(&record).unwrap())
        .await;
    assert!(response.is_ok(), "gRPC trace insert should succeed");

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
    assert!(
        !page.items.is_empty(),
        "Trace record should appear after gRPC ingest"
    );
}

#[tokio::test]
async fn test_grpc_insert_tag_record() {
    let helper = setup_test().await;
    let client = helper.create_grpc_client().await;

    let entity_id = create_uuid7();
    let tag = TagRecord {
        entity_id: entity_id.clone(),
        entity_type: "pipeline".to_string(),
        key: "owner".to_string(),
        value: "ml-team".to_string(),
    };

    let record = MessageRecord::TagServerRecord(tag);
    let response = client
        .insert_message(serde_json::to_vec(&record).unwrap())
        .await;
    assert!(response.is_ok(), "gRPC tag insert should succeed");

    sleep(Duration::from_secs(2)).await;

    let tag_request = TagsRequest {
        entity_type: "pipeline".to_string(),
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
    assert_eq!(
        tags_response.tags.len(),
        1,
        "Tag should be present after gRPC ingest"
    );
    assert_eq!(tags_response.tags[0].value, "ml-team");
}

#[tokio::test]
async fn test_grpc_insert_invalid_message() {
    let helper = setup_test().await;
    let client = helper.create_grpc_client().await;

    let response = client.insert_message(b"not valid json".to_vec()).await;

    assert!(response.is_err(), "Invalid bytes should return an error");
    // GrpcClient wraps tonic::Status into ClientError::GrpcError — verify the error
    // string contains the expected status code so the deserialization error path is exercised.
    let err_str = format!("{:?}", response.unwrap_err());
    assert!(
        err_str.contains("InvalidArgument") || err_str.contains("invalid"),
        "Expected InvalidArgument gRPC status for malformed payload, got: {err_str}"
    );
}
