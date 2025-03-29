use crate::common::TestHelper;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use http_body_util::BodyExt;
use scouter_server::api::routes::user::schema::CreateUserRequest;
use scouter_server::api::routes::user::schema::UpdateUserRequest;
use scouter_server::api::routes::user::schema::UserListResponse;
use scouter_server::api::routes::user::schema::UserResponse;

#[tokio::test]
async fn test_server_user_crud() {
    let helper = TestHelper::new(false, false).await.unwrap();

    // 1. Create a new user
    let create_req = CreateUserRequest {
        username: "test_user".to_string(),
        password: "test_password".to_string(),
        permissions: Some(vec!["read".to_string(), "write".to_string()]),
        group_permissions: Some(vec!["user".to_string()]),
        role: Some("user".to_string()),
        active: Some(true),
    };

    let body = serde_json::to_string(&create_req).unwrap();

    let request = Request::builder()
        .uri("/scouter/users")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let user_response: UserResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(user_response.username, "test_user");
    assert_eq!(
        user_response.permissions,
        vec!["read".to_string(), "write".to_string()]
    );
    assert_eq!(user_response.group_permissions, vec!["user".to_string()]);
    assert!(user_response.active);

    // 2. Get the user
    let request = Request::builder()
        .uri("/scouter/users/test_user")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let user_response: UserResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(user_response.username, "test_user");

    // 3. Update the user
    let update_req = UpdateUserRequest {
        password: Some("new_password".to_string()),
        permissions: Some(vec![
            "read".to_string(),
            "write".to_string(),
            "execute".to_string(),
        ]),
        group_permissions: Some(vec!["user".to_string(), "developer".to_string()]),
        active: Some(true),
    };

    let body = serde_json::to_string(&update_req).unwrap();

    let request = Request::builder()
        .uri("/scouter/users/test_user")
        .method("PUT")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let user_response: UserResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(
        user_response.permissions,
        vec![
            "read".to_string(),
            "write".to_string(),
            "execute".to_string()
        ]
    );
    assert_eq!(
        user_response.group_permissions,
        vec!["user".to_string(), "developer".to_string()]
    );

    // 4. List all users
    let request = Request::builder()
        .uri("/scouter/users")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let list_response: UserListResponse = serde_json::from_slice(&body).unwrap();

    // Find our test user in the list
    let test_user = list_response
        .users
        .iter()
        .find(|u| u.username == "test_user");
    assert!(test_user.is_some());

    // 5. Delete the user
    let request = Request::builder()
        .uri("/scouter/users/test_user")
        .method("DELETE")
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::OK);

    // Verify the user is deleted by trying to get it
    let request = Request::builder()
        .uri("/scouter/users/test_user")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
