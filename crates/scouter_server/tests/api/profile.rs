use crate::common::{TestHelper, NAME, SPACE};

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};

use crate::common::VERSION;
use http_body_util::BodyExt;
use scouter_drift::spc::SpcMonitor;
use scouter_types::custom::CustomMetricDriftConfig;
use scouter_types::{
    contracts::{GetProfileRequest, ProfileStatusRequest},
    ListProfilesRequest,
};
use scouter_types::{custom::CustomDriftProfile, spc::SpcAlertConfig};
use scouter_types::{custom::CustomMetric, spc::SpcDriftConfig, AlertThreshold};
use scouter_types::{custom::CustomMetricAlertConfig, ListedProfile};
use scouter_types::{DriftType, RegisteredProfileResponse};
#[tokio::test]
async fn test_create_spc_profile() {
    let helper = TestHelper::new(false, false).await.unwrap();

    let (array, features) = helper.get_data();
    let alert_config = SpcAlertConfig::default();
    let config = SpcDriftConfig::new(SPACE, NAME, VERSION, None, None, Some(alert_config), None);
    let monitor = SpcMonitor::new();

    let mut profile = monitor
        .create_2d_drift_profile(&features, &array.view(), &config.unwrap())
        .unwrap();

    let _uid = helper
        .register_drift_profile(profile.create_profile_request().unwrap())
        .await;

    // update profile
    profile.config.sample_size = 100;

    assert_eq!(profile.config.sample_size, 100);

    let request = profile.create_profile_request().unwrap();

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
        .uri(format!("/scouter/profile?{query_string}"))
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
async fn test_profile_versions() {
    let helper = TestHelper::new(false, false).await.unwrap();
    let metrics = CustomMetric::new("mae", 10.0, AlertThreshold::Below, None).unwrap();
    let alert_config = CustomMetricAlertConfig::default();
    let config =
        CustomMetricDriftConfig::new(SPACE, NAME, VERSION, 25, alert_config, None).unwrap();
    let profile = CustomDriftProfile::new(config, vec![metrics]).unwrap();

    let request = profile.create_profile_request().unwrap();
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
    // deserialise for RegisteredProfileResponse

    let body = response.into_body().collect().await.unwrap().to_bytes();

    let results: RegisteredProfileResponse = serde_json::from_slice(&body).unwrap();

    assert_eq!(results.version, "1.0.0");

    // do it again
    let metrics = CustomMetric::new("mae", 10.0, AlertThreshold::Below, None).unwrap();
    let alert_config = CustomMetricAlertConfig::default();
    let config =
        CustomMetricDriftConfig::new(SPACE, NAME, VERSION, 25, alert_config, None).unwrap();
    let profile = CustomDriftProfile::new(config, vec![metrics]).unwrap();

    let mut request = profile.create_profile_request().unwrap();
    request.active = true;
    request.deactivate_others = true;
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
    // deserialise for RegisteredProfileResponse

    let body = response.into_body().collect().await.unwrap().to_bytes();

    let results: RegisteredProfileResponse = serde_json::from_slice(&body).unwrap();

    assert_eq!(results.version, "1.1.0");
    assert!(results.active);

    // list profiles
    let list_request = ListProfilesRequest {
        space: SPACE.to_string(),
        name: NAME.to_string(),
        version: results.version,
    };

    let request = Request::builder()
        .uri("/scouter/profiles")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(&list_request).unwrap()))
        .unwrap();

    let response = helper.send_oneshot(request).await;

    //assert response
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();

    let results: Vec<ListedProfile> = serde_json::from_slice(&body).unwrap();

    assert_eq!(results.len(), 1);
    assert!(results[0].active);
}
