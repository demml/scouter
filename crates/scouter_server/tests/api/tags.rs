use crate::common::TestHelper;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use chrono::Utc;
use http_body_util::BodyExt;
use potato_head::create_uuid7;
use scouter_types::{InsertTagRequest, TagRecord, TagRequest, TagsResponse};

#[tokio::test]
async fn test_tags() {
    let helper = TestHelper::new(false, false).await.unwrap();

    let uid = create_uuid7();

    let tag1 = TagRecord {
        created_at: Utc::now(),
        entity_id: uid.clone(),
        entity_type: "service".to_string(),
        key: "env".to_string(),
        value: "production".to_string(),
    };

    let tag2 = TagRecord {
        created_at: Utc::now(),
        entity_id: uid.clone(),
        entity_type: "service".to_string(),
        key: "version".to_string(),
        value: "1.0.0".to_string(),
    };
    let tag_record = InsertTagRequest {
        tags: vec![tag1, tag2],
    };

    let body = serde_json::to_string(&tag_record).unwrap();

    let request = Request::builder()
        .uri("/scouter/tags")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);

    // retrieve tags
    let tag_request = TagRequest {
        entity_type: "service".to_string(),
        entity_id: uid.clone(),
    };

    let query_string = serde_qs::to_string(&tag_request).unwrap();
    let request = Request::builder()
        .uri(format!("/scouter/tags?{query_string}"))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let tags_response: TagsResponse = serde_json::from_slice(&body).unwrap();

    assert_eq!(tags_response.tags.len(), 2, "Should return 2 tags");
}
