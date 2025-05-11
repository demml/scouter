use crate::common::{TestHelper, NAME, SPACE, VERSION};

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};

use scouter_drift::spc::SpcMonitor;
use scouter_types::spc::SpcAlertConfig;
use scouter_types::spc::SpcDriftConfig;
use scouter_types::DriftType;

use http_body_util::BodyExt;
use scouter_drift::psi::PsiMonitor;
use scouter_types::contracts::{
    DriftRequest, GetProfileRequest, ProfileRequest, ProfileStatusRequest,
};
use scouter_types::custom::{
    AlertThreshold, BinnedCustomMetrics, CustomDriftProfile, CustomMetric, CustomMetricAlertConfig,
    CustomMetricDriftConfig,
};
use scouter_types::psi::BinnedPsiFeatureMetrics;
use scouter_types::psi::{PsiAlertConfig, PsiDriftConfig};
use scouter_types::spc::SpcDriftFeatures;
use scouter_types::TimeInterval;

#[tokio::test]
async fn test_create_spc_profile() {
    let helper = TestHelper::new(false, false).await.unwrap();

    let (array, features) = helper.get_data();
    let alert_config = SpcAlertConfig::default();
    let config = SpcDriftConfig::new(
        Some(SPACE.to_string()),
        Some(NAME.to_string()),
        None,
        None,
        None,
        Some(alert_config),
        None,
    );

    let monitor = SpcMonitor::new();

    let mut profile = monitor
        .create_2d_drift_profile(&features, &array.view(), &config.unwrap())
        .unwrap();

    let request = ProfileRequest {
        space: profile.config.space.clone(),
        profile: profile.model_dump_json(),
        drift_type: DriftType::Spc,
    };

    let body = serde_json::to_string(&request).unwrap();

    let request = Request::builder()
        .uri("/scouter/profile")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = helper.send_oneshot(request).await;

    //assert response
    assert_eq!(response.status(), StatusCode::OK);

    // update profile
    profile.config.sample_size = 100;

    assert_eq!(profile.config.sample_size, 100);

    let request = ProfileRequest {
        space: profile.config.space.clone(),
        profile: profile.model_dump_json(),
        drift_type: DriftType::Spc,
    };

    let body = serde_json::to_string(&request).unwrap();

    let request = Request::builder()
        .uri("/scouter/profile")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = helper.send_oneshot(request).await;

    //assert response
    assert_eq!(response.status(), StatusCode::OK);

    // get profile
    let params = GetProfileRequest {
        name: profile.config.name.clone(),
        space: profile.config.space.clone(),
        version: profile.config.version.clone(),
        drift_type: DriftType::Spc,
    };

    let query_string = serde_qs::to_string(&params).unwrap();

    let request = Request::builder()
        .uri(format!("/scouter/profile?{}", query_string))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;

    //assert response
    assert_eq!(response.status(), StatusCode::OK);

    // update profile status
    let request = ProfileStatusRequest {
        name: profile.config.name.clone(),
        space: profile.config.space.clone(),
        version: profile.config.version.clone(),
        active: true,
        drift_type: None,
        deactivate_others: true,
    };

    let body = serde_json::to_string(&request).unwrap();

    let request = Request::builder()
        .uri("/scouter/profile/status")
        .method("PUT")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = helper.send_oneshot(request).await;

    //assert response
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_spc_server_records() {
    let helper = TestHelper::new(false, false).await.unwrap();
    let records = helper.get_spc_drift_records(None);
    let body = serde_json::to_string(&records).unwrap();

    let request = Request::builder()
        .uri("/scouter/drift")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = helper.send_oneshot(request).await;

    //assert response
    assert_eq!(response.status(), StatusCode::OK);

    // get drift records
    let params = DriftRequest {
        space: SPACE.to_string(),
        name: NAME.to_string(),
        version: VERSION.to_string(),
        time_interval: TimeInterval::FiveMinutes,
        max_data_points: 100,
        drift_type: DriftType::Spc,
        ..Default::default()
    };

    let query_string = serde_qs::to_string(&params).unwrap();

    let request = Request::builder()
        .uri(format!("/scouter/drift/spc?{}", query_string))
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
    let helper = TestHelper::new(false, false).await.unwrap();

    let (array, features) = helper.get_data();
    let alert_config = PsiAlertConfig {
        features_to_monitor: vec![
            "feature_1".to_string(),
            "feature_2".to_string(),
            "feature_3".to_string(),
        ],
        ..Default::default()
    };

    let config = PsiDriftConfig::new(SPACE, NAME, VERSION, alert_config, None);

    let monitor = PsiMonitor::new();

    let profile = monitor
        .create_2d_drift_profile(&features, &array.view(), &config.unwrap())
        .unwrap();

    let request = ProfileRequest {
        space: profile.config.space.clone(),
        profile: profile.model_dump_json(),
        drift_type: DriftType::Psi,
    };

    let body = serde_json::to_string(&request).unwrap();

    let request = Request::builder()
        .uri("/scouter/profile")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = helper.send_oneshot(request).await;

    //assert response
    assert_eq!(response.status(), StatusCode::OK);

    let records = helper.get_psi_drift_records(None);
    let body = serde_json::to_string(&records).unwrap();

    let request = Request::builder()
        .uri("/scouter/drift")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = helper.send_oneshot(request).await;

    //assert response
    assert_eq!(response.status(), StatusCode::OK);

    // get drift records
    let params = DriftRequest {
        space: SPACE.to_string(),
        name: NAME.to_string(),
        version: VERSION.to_string(),
        time_interval: TimeInterval::FiveMinutes,
        max_data_points: 100,
        drift_type: DriftType::Psi,
        ..Default::default()
    };

    let query_string = serde_qs::to_string(&params).unwrap();

    let request = Request::builder()
        .uri(format!("/scouter/drift/psi?{}", query_string))
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
    let helper = TestHelper::new(false, false).await.unwrap();

    let alert_config = CustomMetricAlertConfig::default();
    let config =
        CustomMetricDriftConfig::new(SPACE, NAME, VERSION, 25, alert_config, None).unwrap();

    let alert_threshold = AlertThreshold::Above;
    let metric1 = CustomMetric::new("metric1", 1.0, alert_threshold.clone(), None).unwrap();
    let metric2 = CustomMetric::new("metric2", 1.0, alert_threshold, None).unwrap();
    let profile = CustomDriftProfile::new(config, vec![metric1, metric2], None).unwrap();

    let request = ProfileRequest {
        space: profile.config.space.clone(),
        profile: profile.model_dump_json(),
        drift_type: DriftType::Custom,
    };

    let body = serde_json::to_string(&request).unwrap();
    let request = Request::builder()
        .uri("/scouter/profile")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = helper.send_oneshot(request).await;

    //assert response
    assert_eq!(response.status(), StatusCode::OK);

    let records = helper.get_custom_drift_records(None);
    let body = serde_json::to_string(&records).unwrap();

    let request = Request::builder()
        .uri("/scouter/drift")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = helper.send_oneshot(request).await;

    //assert response
    assert_eq!(response.status(), StatusCode::OK);

    // get drift records
    let params = DriftRequest {
        space: SPACE.to_string(),
        name: NAME.to_string(),
        version: VERSION.to_string(),
        time_interval: TimeInterval::FiveMinutes,
        max_data_points: 100,
        drift_type: DriftType::Custom,
        ..Default::default()
    };

    let query_string = serde_qs::to_string(&params).unwrap();

    let request = Request::builder()
        .uri(format!("/scouter/drift/custom?{}", query_string))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;

    //assert response
    assert_eq!(response.status(), StatusCode::OK);

    // collect body into serde Value

    let val = response.into_body().collect().await.unwrap().to_bytes();

    let results: BinnedCustomMetrics = serde_json::from_slice(&val).unwrap();

    assert!(!results.metrics.is_empty());
}
