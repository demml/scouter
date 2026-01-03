use crate::common::{setup_test, TestHelper, NAME, SPACE, VERSION};
use std::time::Duration;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use potato_head::mock::LLMTestServer;

use http_body_util::BodyExt;
use scouter_drift::psi::PsiMonitor;
use scouter_types::custom::{
    CustomDriftProfile, CustomMetric, CustomMetricAlertConfig, CustomMetricDriftConfig,
};
use scouter_types::psi::BinnedPsiFeatureMetrics;
use scouter_types::psi::{PsiAlertConfig, PsiDriftConfig};
use scouter_types::spc::SpcDriftFeatures;
use scouter_types::{contracts::DriftRequest, GenAIEventRecordPaginationResponse};
use scouter_types::{
    AlertThreshold, BinnedMetrics, GenAIEventRecordPaginationRequest, ServiceInfo, TimeInterval,
};
use tokio::time::sleep;

#[tokio::test]
async fn test_spc_server_records() {
    let helper = setup_test().await;
    let profile = helper.create_drift_profile().await;
    let records = helper.get_spc_drift_records(None, &profile.config.uid);
    let body = serde_json::to_string(&records).unwrap();

    let request = Request::builder()
        .uri("/scouter/message")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = helper.send_oneshot(request).await;

    // Sleep for 2 seconds to allow the http consumer time to process all server records sent above.
    sleep(Duration::from_secs(2)).await;

    //assert response
    assert_eq!(response.status(), StatusCode::OK);

    // get drift records
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

    //assert response
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();

    let results: SpcDriftFeatures = serde_json::from_slice(&body).unwrap();

    assert_eq!(results.features.len(), 10);
}

#[tokio::test]
async fn test_psi_server_records() {
    let helper = setup_test().await;

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

    let records = helper.get_psi_drift_records(None, &uid);
    let body = serde_json::to_string(&records).unwrap();

    let request = Request::builder()
        .uri("/scouter/message")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = helper.send_oneshot(request).await;

    //assert response
    assert_eq!(response.status(), StatusCode::OK);

    // Sleep for 2 seconds to allow the http consumer time to process all server records sent above.
    sleep(Duration::from_secs(2)).await;

    // get drift records
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

    //assert response
    assert_eq!(response.status(), StatusCode::OK);

    // collect body into serde Value

    let val = response.into_body().collect().await.unwrap().to_bytes();

    let results: BinnedPsiFeatureMetrics = serde_json::from_slice(&val).unwrap();

    assert!(!results.features.is_empty());
}

#[tokio::test]
async fn test_custom_server_records() {
    let helper = setup_test().await;

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

    let records = helper.get_custom_drift_records(None, &uid);
    let body = serde_json::to_string(&records).unwrap();

    let request = Request::builder()
        .uri("/scouter/message")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = helper.send_oneshot(request).await;

    //assert response
    assert_eq!(response.status(), StatusCode::OK);

    // Sleep for 2 seconds to allow the http consumer time to process all server records sent above.
    sleep(Duration::from_secs(2)).await;

    // get drift records
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

    //assert response
    assert_eq!(response.status(), StatusCode::OK);

    // collect body into serde Value

    let val = response.into_body().collect().await.unwrap().to_bytes();

    let results: BinnedMetrics = serde_json::from_slice(&val).unwrap();

    assert!(!results.metrics.is_empty());
}

#[test]
fn test_genai_server_records() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut mock = LLMTestServer::new();
    mock.start_server().unwrap();

    let helper = runtime.block_on(async { setup_test().await });
    let profile = runtime.block_on(async { TestHelper::create_genai_drift_profile().await });

    let uid = runtime.block_on(async {
        helper
            .register_drift_profile(profile.create_profile_request().unwrap())
            .await
    });

    // populate the server with GenAI drift records
    let records = helper.get_genai_event_records(None, &uid);

    let body = serde_json::to_string(&records).unwrap();
    //
    let request = Request::builder()
        .uri("/scouter/message")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = runtime.block_on(async { helper.send_oneshot(request).await });

    assert_eq!(response.status(), StatusCode::OK);
    //
    //// Sleep for 2 seconds to allow the http consumer time to process all server records sent above.
    runtime.block_on(async { sleep(Duration::from_secs(5)).await });

    // get drift records
    let params = DriftRequest {
        space: SPACE.to_string(),
        uid: uid.clone(),
        time_interval: TimeInterval::FifteenMinutes,
        max_data_points: 100,
        ..Default::default()
    };

    // Test getting binned task metrics
    let query_string = serde_qs::to_string(&params).unwrap();
    let request = Request::builder()
        .uri(format!("/scouter/drift/genai/task?{query_string}"))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = runtime.block_on(async { helper.send_oneshot(request).await });
    //assert response
    assert_eq!(response.status(), StatusCode::OK);
    // collect body into serde Value
    let val = runtime.block_on(async { response.into_body().collect().await.unwrap().to_bytes() });
    let results: BinnedMetrics = serde_json::from_slice(&val).unwrap();
    assert!(!results.metrics.is_empty());

    // Test getting binned workflow metric
    let query_string = serde_qs::to_string(&params).unwrap();
    let request = Request::builder()
        .uri(format!("/scouter/drift/genai/workflow?{query_string}"))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = runtime.block_on(async { helper.send_oneshot(request).await });
    //assert response
    assert_eq!(response.status(), StatusCode::OK);
    // collect body into serde Value
    let val = runtime.block_on(async { response.into_body().collect().await.unwrap().to_bytes() });
    let results: BinnedMetrics = serde_json::from_slice(&val).unwrap();
    assert!(!results.metrics.is_empty());

    // get drift records by page
    let request = GenAIEventRecordPaginationRequest {
        service_info: ServiceInfo {
            space: SPACE.to_string(),
            uid: uid.clone(),
        },
        status: None,
        limit: Some(10),
        ..Default::default()
    };

    let body = serde_json::to_string(&request).unwrap();

    let request = Request::builder()
        .uri("/scouter/drift/genai/records")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();
    let response = runtime.block_on(async { helper.send_oneshot(request).await });
    let val = runtime.block_on(async { response.into_body().collect().await.unwrap().to_bytes() });

    let records: GenAIEventRecordPaginationResponse = serde_json::from_slice(&val).unwrap();
    assert!(!records.items.is_empty());
    assert!(records.has_next);

    mock.stop_server().unwrap();
    TestHelper::cleanup_storage();
}
