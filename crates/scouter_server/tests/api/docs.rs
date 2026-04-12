use crate::common::TestHelper;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;

#[tokio::test]
async fn test_list_docs() {
    let helper = TestHelper::new(false, false).await.unwrap();

    let request = Request::builder()
        .uri("/scouter/api/v1/docs")
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let docs = v["docs"].as_array().expect("docs should be an array");
    assert!(!docs.is_empty(), "should return at least one doc");

    // Verify required fields on each entry
    for doc in docs {
        assert!(doc["id"].as_str().is_some(), "each doc must have an id");
        assert!(
            doc["title"].as_str().is_some(),
            "each doc must have a title"
        );
        assert!(
            doc["category"].as_str().is_some(),
            "each doc must have a category"
        );
    }

    // Spot-check known entries
    let ids: Vec<&str> = docs.iter().filter_map(|d| d["id"].as_str()).collect();
    assert!(ids.contains(&"index"), "should include index doc");
    assert!(
        ids.contains(&"agents/overview"),
        "should include agents/overview"
    );
    assert!(
        ids.contains(&"monitoring/psi/quickstart"),
        "should include monitoring/psi/quickstart"
    );
}

#[tokio::test]
async fn test_get_doc_by_id() {
    let helper = TestHelper::new(false, false).await.unwrap();

    let request = Request::builder()
        .uri("/scouter/api/v1/docs/agents/overview")
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(v["id"], "agents/overview");
    assert_eq!(v["category"], "agents");
    assert!(
        v["content"]
            .as_str()
            .map(|s| !s.is_empty())
            .unwrap_or(false),
        "content should be non-empty markdown"
    );
}

#[tokio::test]
async fn test_get_doc_not_found() {
    let helper = TestHelper::new(false, false).await.unwrap();

    let request = Request::builder()
        .uri("/scouter/api/v1/docs/does/not/exist")
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(v["code"], "NOT_FOUND");
    assert!(
        v["suggested_action"].as_str().is_some(),
        "not-found error should include a suggested_action"
    );
}

#[tokio::test]
async fn test_search_docs() {
    let helper = TestHelper::new(false, false).await.unwrap();

    let request = Request::builder()
        .uri("/scouter/api/v1/docs/search?q=drift")
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(v["query"], "drift");
    let results = v["results"].as_array().expect("results should be an array");
    assert!(
        !results.is_empty(),
        "searching 'drift' should return results"
    );

    for result in results {
        assert!(result["id"].as_str().is_some(), "result must have id");
        assert!(
            result["snippet"].as_str().is_some(),
            "result must have snippet"
        );
    }
}

#[tokio::test]
async fn test_search_docs_query_too_long() {
    let helper = TestHelper::new(false, false).await.unwrap();

    let long_query = "x".repeat(201);
    let uri = format!("/scouter/api/v1/docs/search?q={long_query}");
    let request = Request::builder().uri(uri).body(Body::empty()).unwrap();

    let response = helper.send_oneshot(request).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(v["code"], "BAD_REQUEST");
}
