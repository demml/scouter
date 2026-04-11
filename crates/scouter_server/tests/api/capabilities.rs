use crate::common::TestHelper;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;

#[tokio::test]
async fn test_capabilities() {
    let helper = TestHelper::new(false, false).await.unwrap();

    let request = Request::builder()
        .uri("/scouter/api/v1/capabilities")
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(v["api_version"], "1.0.0", "api_version should be 1.0.0");
    assert_eq!(
        v["auth"]["auth_type"], "bearer",
        "auth_type should be bearer"
    );
    assert!(
        v["auth"]["login_endpoint"].as_str().is_some(),
        "login_endpoint should be present"
    );
    assert!(
        v["endpoints"]["openapi_spec"].as_str().is_some(),
        "openapi_spec endpoint should be present"
    );
    assert!(
        v["features"]["drift_detection"].as_bool().unwrap_or(false),
        "drift_detection feature should be enabled"
    );
}

#[tokio::test]
async fn test_openapi_json() {
    let helper = TestHelper::new(false, false).await.unwrap();

    let request = Request::builder()
        .uri("/scouter/api/v1/openapi.json")
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(
        v["info"]["version"], "1.0.0",
        "OpenAPI info.version should be 1.0.0"
    );
    assert!(
        v["paths"]["/scouter/healthcheck"].is_object(),
        "OpenAPI spec should include /scouter/healthcheck path"
    );
    assert!(
        v["paths"]["/scouter/drift/spc"].is_object(),
        "OpenAPI spec should include /scouter/drift/spc path"
    );
    assert!(
        v["components"]["schemas"]["ScouterServerError"].is_object(),
        "OpenAPI spec should include ScouterServerError schema"
    );
}

/// Verify that error responses include machine-readable `code` and human-readable `error` fields.
/// Uses the 200-char input validation gate (no DB required) to reliably produce a 400.
#[tokio::test]
async fn test_error_response_has_code() {
    let helper = TestHelper::new(false, false).await.unwrap();

    // `name` param is 201 bytes — triggers the 200-char validation guard in get_profile
    let long_name = "a".repeat(201);
    let uri = format!(
        "/scouter/profile?name={long_name}&space=test&version=1.0.0&drift_type=spc"
    );
    let request = Request::builder()
        .uri(uri)
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;

    assert_eq!(
        response.status(),
        StatusCode::BAD_REQUEST,
        "Oversized param should return 400"
    );

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert!(
        v["error"].as_str().map(|s| !s.is_empty()).unwrap_or(false),
        "error field should be present and non-empty"
    );
    assert_eq!(v["code"], "BAD_REQUEST", "code field should be BAD_REQUEST");
}
