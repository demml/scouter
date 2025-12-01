use crate::common::TestHelper;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use http_body_util::BodyExt;
use scouter_types::alert::Alerts;
use scouter_types::contracts::{DriftAlertRequest, UpdateAlertStatus};
use scouter_types::DriftType;

#[tokio::test]
async fn test_get_drift_alerts() {
    let helper = TestHelper::new(false, false).await.unwrap();
    helper.insert_alerts().await.unwrap();

    let uid = helper
        .get_uid_from_args("repo_1", "model_1", "1.0.0", &DriftType::Spc)
        .await
        .unwrap();

    let request = DriftAlertRequest {
        uid: uid,
        limit_datetime: None,
        limit: None,
        active: Some(true),
    };

    let query_string = serde_qs::to_string(&request).unwrap();

    let request = Request::builder()
        .uri(format!("/scouter/alerts?{query_string}"))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;

    //assert response
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();

    let results: Alerts = serde_json::from_slice(&body).unwrap();

    assert!(results.alerts.len() > 1);

    // update alert status
    let request = UpdateAlertStatus {
        id: results.alerts[0].id,
        active: false,
        space: "repo_1".to_string(),
    };

    // put request

    let body = serde_json::to_string(&request).unwrap();
    let request = Request::builder()
        .uri("/scouter/alerts")
        .method("PUT")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = helper.send_oneshot(request).await;
    //assert response
    assert_eq!(response.status(), StatusCode::OK);
}
