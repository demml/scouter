use crate::common::setup_test;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use http_body_util::BodyExt;
use potato_head::create_uuid7;
use scouter_types::{InsertTagsRequest, Tag, TagRecord, TagsRequest, TagsResponse};

#[tokio::test]
async fn test_tags() {
    let helper = setup_test().await;

    let uid = create_uuid7();

    let tag1 = TagRecord {
        entity_id: uid.clone(),
        entity_type: "service".to_string(),
        key: "env".to_string(),
        value: "production".to_string(),
    };

    let tag2 = TagRecord {
        entity_id: uid.clone(),
        entity_type: "service".to_string(),
        key: "version".to_string(),
        value: "1.0.0".to_string(),
    };
    let tag_record = InsertTagsRequest {
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
    let tag_request = TagsRequest {
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

    // retrieve entity ID by tags
    let entity_id_request = scouter_types::EntityIdTagsRequest {
        entity_type: "service".to_string(),
        tags: vec![Tag {
            key: "env".to_string(),
            value: "production".to_string(),
        }],
        match_all: true,
    };

    let query_string = serde_qs::to_string(&entity_id_request).unwrap();
    let request = Request::builder()
        .uri(format!("/scouter/tags/entity_id?{query_string}"))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let entity_id_response: scouter_types::EntityIdTagsResponse =
        serde_json::from_slice(&body).unwrap();

    assert_eq!(
        entity_id_response.entity_id,
        vec![uid],
        "Should return the correct entity ID"
    );
}
