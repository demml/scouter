use crate::common::{setup_test, TestHelper, SPACE};
use std::time::Duration;

use axum::{
    body::Body,
    http::{header, Request},
};
use http_body_util::BodyExt;
use potato_head::mock::LLMTestServer;
use scouter_types::{
    GenAIEvalRecordPaginationRequest, GenAIEvalTaskResponse, RecordType, ServiceInfo,
};
use scouter_types::{GenAIEvalRecordPaginationResponse, GenAIEvalWorkflowPaginationResponse};
use tokio::time::sleep;

#[test]
fn test_genai_server_records() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut mock = LLMTestServer::new();
    mock.start_server().unwrap();

    let helper = runtime.block_on(async { setup_test().await });
    let profile = runtime.block_on(async { TestHelper::create_genai_drift_profile().await });

    let uid = runtime.block_on(async {
        helper
            .register_drift_profile(profile.create_profile_request().unwrap())
            .await
    });

    // populate the server with GenAI tasks and workflow records
    helper.populate_genai_records(&uid, &runtime, None, RecordType::GenAIEval);
    helper.populate_genai_records(&uid, &runtime, None, RecordType::GenAITask);
    helper.populate_genai_records(&uid, &runtime, None, RecordType::GenAIWorkflow);
    //
    //// Sleep for 2 seconds to allow the http consumer time to process all server records sent above.
    runtime.block_on(async { sleep(Duration::from_secs(5)).await });

    // get drift records by page
    let request = GenAIEvalRecordPaginationRequest {
        service_info: ServiceInfo {
            space: SPACE.to_string(),
            uid: uid.clone(),
        },
        status: None,
        limit: Some(10),
        ..Default::default()
    };

    let body = serde_json::to_string(&request).unwrap();

    // get paginated GenAI eval records
    let request = Request::builder()
        .uri("/scouter/genai/page/record")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.clone()))
        .unwrap();
    let response = runtime.block_on(async { helper.send_oneshot(request).await });
    let val = runtime.block_on(async { response.into_body().collect().await.unwrap().to_bytes() });

    let records: GenAIEvalRecordPaginationResponse = serde_json::from_slice(&val).unwrap();
    assert!(!records.items.is_empty());
    assert!(records.has_next);

    // get paginated GenAI workflow records
    let request = Request::builder()
        .uri("/scouter/genai/page/workflow")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.clone()))
        .unwrap();
    let response = runtime.block_on(async { helper.send_oneshot(request).await });
    let val = runtime.block_on(async { response.into_body().collect().await.unwrap().to_bytes() });
    let records: GenAIEvalWorkflowPaginationResponse = serde_json::from_slice(&val).unwrap();
    assert!(!records.items.is_empty());
    assert!(records.has_next);

    // get first eval task for the first record, get record_uid and get tasks
    let first_record_uid = records.items[0].record_uid.clone();
    let request = Request::builder()
        .uri(format!("/scouter/genai/task?record_uid={first_record_uid}"))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let response = runtime.block_on(async { helper.send_oneshot(request).await });
    // Get response body as bytes
    let val = runtime.block_on(async { response.into_body().collect().await.unwrap().to_bytes() });

    let tasks: GenAIEvalTaskResponse = serde_json::from_slice(&val).unwrap();
    assert!(!tasks.tasks.is_empty());

    mock.stop_server().unwrap();
    TestHelper::cleanup_storage();
}
