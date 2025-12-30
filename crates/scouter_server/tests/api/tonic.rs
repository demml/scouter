use crate::common::{setup_test, NAME, SPACE, VERSION};
use std::time::Duration;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use scouter_drift::psi::PsiMonitor;

use scouter_types::contracts::DriftRequest;
use scouter_types::custom::{
    CustomDriftProfile, CustomMetric, CustomMetricAlertConfig, CustomMetricDriftConfig,
};
use scouter_types::psi::BinnedPsiFeatureMetrics;
use scouter_types::psi::{PsiAlertConfig, PsiDriftConfig};
use scouter_types::spc::SpcDriftFeatures;
use scouter_types::{AlertThreshold, BinnedMetrics, TimeInterval};
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
