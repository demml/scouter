use crate::common::{TestHelper, NAME, SPACE};

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};

use scouter_drift::spc::SpcMonitor;
use scouter_types::spc::SpcAlertConfig;
use scouter_types::spc::SpcDriftConfig;
use scouter_types::DriftType;

use scouter_types::contracts::{GetProfileRequest, ProfileRequest, ProfileStatusRequest};

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
