use crate::common::TestHelper;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use scouter_server::api::routes::health::Alive;

#[tokio::test]
async fn test_health_check() {
    let helper = TestHelper::new(false, false).await.unwrap();

    let request = Request::builder()
        .uri("/scouter/healthcheck")
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;

    //assert response
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();

    let v: Alive = serde_json::from_slice(&body).unwrap();

    assert_eq!(v.status, "Alive");
}
