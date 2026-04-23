use crate::common::setup_test;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::Utc;
use scouter_dataframe::EvalScenarioRecord;

#[test]
fn test_eval_scenarios_route_returns_404_for_missing_collection() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let helper = runtime.block_on(async { setup_test().await });

    let request = Request::builder()
        .uri("/scouter/eval/scenarios?collection_id=missing-collection")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = runtime.block_on(async { helper.send_oneshot(request).await });
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[test]
fn test_eval_scenarios_route_returns_200_for_valid_collection() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let helper = runtime.block_on(async { setup_test().await });

    runtime.block_on(async {
        helper
            .eval_scenario_service
            .write_scenarios(vec![EvalScenarioRecord {
                collection_id: "owner-scope".to_string(),
                scenario_id: "admin-s1".to_string(),
                scenario_json: r#"{"id":"admin-s1","initial_query":"admin"}"#.to_string(),
                created_at: Utc::now(),
            }])
            .await
            .unwrap();
    });

    let request = Request::builder()
        .uri("/scouter/eval/scenarios?collection_id=owner-scope")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = runtime.block_on(async { helper.send_oneshot(request).await });
    assert_eq!(response.status(), StatusCode::OK);
}

#[test]
fn test_eval_scenarios_route_returns_500_for_malformed_scenario_json() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let helper = runtime.block_on(async { setup_test().await });

    runtime.block_on(async {
        helper
            .eval_scenario_service
            .write_scenarios(vec![EvalScenarioRecord {
                collection_id: "malformed".to_string(),
                scenario_id: "bad-s1".to_string(),
                scenario_json: "{invalid-json".to_string(),
                created_at: Utc::now(),
            }])
            .await
            .unwrap();
    });

    let request = Request::builder()
        .uri("/scouter/eval/scenarios?collection_id=malformed")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = runtime.block_on(async { helper.send_oneshot(request).await });
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}
