use crate::common::setup_test;
use arrow::array::{
    Date32Array, Float64Array, StringArray, StringViewArray, TimestampMicrosecondArray,
};
use arrow_array::RecordBatch;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::Datelike;
use http_body_util::BodyExt;
use scouter_dataframe::parquet::bifrost::ipc::{batches_to_ipc_bytes, ipc_bytes_to_batches};
use scouter_types::dataset::schema::{inject_system_columns, json_schema_to_arrow};
use serde_json::Value;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

const CAT: &str = "testcat";
const SCH: &str = "testschema";
const TBL: &str = "testtbl";

fn test_json_schema() -> &'static str {
    r#"{"type":"object","properties":{"score":{"type":"number"},"label":{"type":"string"}},"required":["score","label"]}"#
}

/// Build a 3-row Arrow RecordBatch with the exact schema the server expects:
/// (score: Float64, label: Utf8View) + system columns injected by the server.
fn test_ipc_bytes() -> Vec<u8> {
    let user_schema = json_schema_to_arrow(test_json_schema()).unwrap();
    let schema = Arc::new(inject_system_columns(user_schema).unwrap());

    let now = chrono::Utc::now();
    let epoch_days = now.date_naive().num_days_from_ce() - 719_163;
    let ts = now.timestamp_micros();

    let batch = RecordBatch::try_new(
        Arc::clone(&schema),
        vec![
            Arc::new(Float64Array::from(vec![1.0, 2.0, 3.0])),
            Arc::new(StringViewArray::from(vec!["a", "b", "c"])),
            Arc::new(TimestampMicrosecondArray::from(vec![ts, ts, ts]).with_timezone("UTC")),
            Arc::new(Date32Array::from(vec![epoch_days, epoch_days, epoch_days])),
            Arc::new(StringArray::from(vec!["batch-1", "batch-1", "batch-1"])),
        ],
    )
    .unwrap();

    batches_to_ipc_bytes(std::slice::from_ref(&batch)).unwrap()
}

/// Register `testcat.testschema.testtbl` via HTTP and return the fingerprint.
async fn register_test_dataset(helper: &crate::common::TestHelper) -> String {
    let body = serde_json::json!({
        "catalog": CAT,
        "schema_name": SCH,
        "table": TBL,
        "json_schema": test_json_schema(),
    });
    let request = Request::builder()
        .uri("/scouter/datasets/register")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let resp = helper.send_oneshot(request).await;
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "register_test_dataset failed"
    );
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    v["fingerprint"].as_str().unwrap().to_string()
}

/// Insert the 3-row test batch via HTTP.
async fn insert_test_batch(helper: &crate::common::TestHelper, fingerprint: &str) {
    let ipc = test_ipc_bytes();
    let request = Request::builder()
        .uri(format!("/scouter/datasets/{CAT}/{SCH}/{TBL}/records"))
        .method("POST")
        .header("x-dataset-fingerprint", fingerprint)
        .body(Body::from(ipc))
        .unwrap();
    let resp = helper.send_oneshot(request).await;
    let status = resp.status();
    if status != StatusCode::OK {
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        panic!(
            "insert_test_batch failed ({}): {}",
            status,
            String::from_utf8_lossy(&body)
        );
    }
}

// ── Group 1: Registration & Catalog Listing ─────────────────────────────────

#[tokio::test]
async fn test_http_register_dataset() {
    let helper = setup_test().await;

    let body = serde_json::json!({
        "catalog": CAT,
        "schema_name": SCH,
        "table": TBL,
        "json_schema": test_json_schema(),
    });
    let request = Request::builder()
        .uri("/scouter/datasets/register")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let resp = helper.send_oneshot(request).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["status"], "created");
    assert!(!v["fingerprint"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn test_http_register_dataset_idempotent() {
    let helper = setup_test().await;
    let fingerprint = register_test_dataset(&helper).await;

    // Second registration with the same schema → already_exists with same fingerprint.
    let body = serde_json::json!({
        "catalog": CAT,
        "schema_name": SCH,
        "table": TBL,
        "json_schema": test_json_schema(),
    });
    let request = Request::builder()
        .uri("/scouter/datasets/register")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let resp = helper.send_oneshot(request).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["status"], "already_exists");
    assert_eq!(v["fingerprint"].as_str().unwrap(), fingerprint);
}

#[tokio::test]
async fn test_http_list_datasets_empty() {
    let helper = setup_test().await;

    let request = Request::builder()
        .uri("/scouter/datasets")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = helper.send_oneshot(request).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["datasets"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_http_list_datasets_after_register() {
    let helper = setup_test().await;
    register_test_dataset(&helper).await;

    let request = Request::builder()
        .uri("/scouter/datasets")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = helper.send_oneshot(request).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["datasets"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_http_get_dataset_info() {
    let helper = setup_test().await;
    register_test_dataset(&helper).await;

    let request = Request::builder()
        .uri(format!("/scouter/datasets/{CAT}/{SCH}/{TBL}/info"))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = helper.send_oneshot(request).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["catalog"], CAT);
    assert_eq!(v["schema_name"], SCH);
    assert_eq!(v["table"], TBL);
}

#[tokio::test]
async fn test_http_get_dataset_info_not_found() {
    let helper = setup_test().await;

    let request = Request::builder()
        .uri("/scouter/datasets/no/such/table/info")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = helper.send_oneshot(request).await;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_http_list_catalogs_empty() {
    let helper = setup_test().await;

    let request = Request::builder()
        .uri("/scouter/datasets/catalogs")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = helper.send_oneshot(request).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["catalogs"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_http_list_catalogs_after_register() {
    let helper = setup_test().await;
    register_test_dataset(&helper).await;

    let request = Request::builder()
        .uri("/scouter/datasets/catalogs")
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = helper.send_oneshot(request).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    let catalogs = v["catalogs"].as_array().unwrap();
    assert_eq!(catalogs.len(), 1);
    assert_eq!(catalogs[0]["catalog"], CAT);
}

#[tokio::test]
async fn test_http_list_schemas() {
    let helper = setup_test().await;
    register_test_dataset(&helper).await;

    let request = Request::builder()
        .uri(format!("/scouter/datasets/catalogs/{CAT}/schemas"))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = helper.send_oneshot(request).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    let schemas = v["schemas"].as_array().unwrap();
    assert_eq!(schemas.len(), 1);
    assert_eq!(schemas[0]["schema_name"], SCH);
}

#[tokio::test]
async fn test_http_list_tables() {
    let helper = setup_test().await;
    register_test_dataset(&helper).await;

    let request = Request::builder()
        .uri(format!(
            "/scouter/datasets/catalogs/{CAT}/schemas/{SCH}/tables"
        ))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = helper.send_oneshot(request).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    let tables = v["tables"].as_array().unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0]["table"], TBL);
}

// ── Group 2: Insert + Catalog Detail ─────────────────────────────────────────

#[tokio::test]
async fn test_http_insert_batch() {
    let helper = setup_test().await;
    let fingerprint = register_test_dataset(&helper).await;

    let ipc = test_ipc_bytes();
    let request = Request::builder()
        .uri(format!("/scouter/datasets/{CAT}/{SCH}/{TBL}/records"))
        .method("POST")
        .header("x-dataset-fingerprint", &fingerprint)
        .body(Body::from(ipc))
        .unwrap();
    let resp = helper.send_oneshot(request).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["rows_accepted"], 3);
}

#[tokio::test]
async fn test_http_get_table_detail() {
    let helper = setup_test().await;
    let fingerprint = register_test_dataset(&helper).await;
    insert_test_batch(&helper, &fingerprint).await;
    sleep(Duration::from_secs(3)).await;

    let request = Request::builder()
        .uri(format!(
            "/scouter/datasets/catalogs/{CAT}/schemas/{SCH}/tables/{TBL}"
        ))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = helper.send_oneshot(request).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(!v["columns"].as_array().unwrap().is_empty());
    assert!(v["stats"]["delta_version"].is_number());
}

#[tokio::test]
async fn test_http_preview_table() {
    let helper = setup_test().await;
    let fingerprint = register_test_dataset(&helper).await;
    insert_test_batch(&helper, &fingerprint).await;
    sleep(Duration::from_secs(3)).await;

    let request = Request::builder()
        .uri(format!(
            "/scouter/datasets/catalogs/{CAT}/schemas/{SCH}/tables/{TBL}/preview?max_rows=10"
        ))
        .method("GET")
        .body(Body::empty())
        .unwrap();
    let resp = helper.send_oneshot(request).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["row_count"], 3);
    assert!(!v["columns"].as_array().unwrap().is_empty());
}

// ── Group 3: Query Path ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_http_query_dataset_ipc() {
    let helper = setup_test().await;
    let fingerprint = register_test_dataset(&helper).await;
    insert_test_batch(&helper, &fingerprint).await;
    sleep(Duration::from_secs(3)).await;

    let sql = format!("SELECT score, label FROM {CAT}.{SCH}.{TBL}");
    let body = serde_json::json!({ "sql": sql });
    let request = Request::builder()
        .uri("/scouter/datasets/sql")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let resp = helper.send_oneshot(request).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let batches = ipc_bytes_to_batches(&bytes).unwrap();
    let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
    assert_eq!(total_rows, 3);
}

#[tokio::test]
async fn test_http_execute_query() {
    let helper = setup_test().await;
    let fingerprint = register_test_dataset(&helper).await;
    insert_test_batch(&helper, &fingerprint).await;
    sleep(Duration::from_secs(3)).await;

    let sql = format!("SELECT score, label FROM {CAT}.{SCH}.{TBL}");
    let body = serde_json::json!({ "sql": sql });
    let request = Request::builder()
        .uri("/scouter/datasets/query")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let resp = helper.send_oneshot(request).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["rows"].as_array().unwrap().len(), 3);
    assert!(!v["metadata"]["query_id"].as_str().unwrap().is_empty());
    assert_eq!(v["metadata"]["truncated"], false);
}

#[tokio::test]
async fn test_http_execute_query_invalid_sql() {
    let helper = setup_test().await;

    let body = serde_json::json!({ "sql": "DROP TABLE foo" });
    let request = Request::builder()
        .uri("/scouter/datasets/query")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let resp = helper.send_oneshot(request).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_http_explain_query() {
    let helper = setup_test().await;
    let fingerprint = register_test_dataset(&helper).await;
    insert_test_batch(&helper, &fingerprint).await;
    sleep(Duration::from_secs(3)).await;

    let sql = format!("SELECT score, label FROM {CAT}.{SCH}.{TBL}");
    let body = serde_json::json!({ "sql": sql, "analyze": false });
    let request = Request::builder()
        .uri("/scouter/datasets/query/explain")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let resp = helper.send_oneshot(request).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    let plan_text = v["logical_plan_text"].as_str().unwrap();
    assert!(!plan_text.is_empty());
    assert!(!plan_text.contains("s3://"));
    assert!(!plan_text.contains("gs://"));
}

#[tokio::test]
async fn test_http_cancel_query_nonexistent() {
    let helper = setup_test().await;

    let body = serde_json::json!({ "query_id": "no-such-id" });
    let request = Request::builder()
        .uri("/scouter/datasets/query/cancel")
        .method("POST")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap();
    let resp = helper.send_oneshot(request).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["cancelled"], false);
}

// ── Group 4: gRPC Parity ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_grpc_register_and_list() {
    let helper = setup_test().await;
    let mut client = helper.create_dataset_grpc_client().await;

    let resp = client
        .register_dataset(CAT, SCH, TBL, test_json_schema(), vec![])
        .await
        .unwrap();
    assert_eq!(resp.status, "created");
    assert!(!resp.fingerprint.is_empty());

    let list = client.list_datasets().await.unwrap();
    assert_eq!(list.datasets.len(), 1);
    assert_eq!(list.datasets[0].catalog, CAT);
}

#[tokio::test]
async fn test_grpc_insert_and_query() {
    let helper = setup_test().await;
    let mut client = helper.create_dataset_grpc_client().await;

    let reg = client
        .register_dataset(CAT, SCH, TBL, test_json_schema(), vec![])
        .await
        .unwrap();
    client
        .insert_batch(CAT, SCH, TBL, &reg.fingerprint, test_ipc_bytes())
        .await
        .unwrap();
    sleep(Duration::from_secs(3)).await;

    let sql = format!("SELECT score, label FROM {CAT}.{SCH}.{TBL}");
    let qr = client.query_dataset(&sql).await.unwrap();
    let batches = ipc_bytes_to_batches(&qr.ipc_data).unwrap();
    let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
    assert_eq!(total_rows, 3);
}

#[tokio::test]
async fn test_grpc_catalog_browser() {
    let helper = setup_test().await;
    let mut client = helper.create_dataset_grpc_client().await;

    client
        .register_dataset(CAT, SCH, TBL, test_json_schema(), vec![])
        .await
        .unwrap();

    let cats = client.list_catalogs().await.unwrap();
    assert_eq!(cats.catalogs.len(), 1);
    assert_eq!(cats.catalogs[0].catalog, CAT);

    let schemas = client.list_schemas(CAT).await.unwrap();
    assert_eq!(schemas.schemas.len(), 1);
    assert_eq!(schemas.schemas[0].schema_name, SCH);

    let tables = client.list_tables(CAT, SCH).await.unwrap();
    assert_eq!(tables.tables.len(), 1);
    assert_eq!(tables.tables[0].table, TBL);
}

#[tokio::test]
async fn test_grpc_execute_query() {
    let helper = setup_test().await;
    let mut client = helper.create_dataset_grpc_client().await;

    let reg = client
        .register_dataset(CAT, SCH, TBL, test_json_schema(), vec![])
        .await
        .unwrap();
    client
        .insert_batch(CAT, SCH, TBL, &reg.fingerprint, test_ipc_bytes())
        .await
        .unwrap();
    sleep(Duration::from_secs(3)).await;

    let sql = format!("SELECT score, label FROM {CAT}.{SCH}.{TBL}");
    let result = client.execute_query(&sql, "q1", 100).await.unwrap();
    let meta = result.metadata.unwrap();
    assert!(meta.rows_returned > 0);
}

#[tokio::test]
async fn test_grpc_explain_query() {
    let helper = setup_test().await;
    let mut client = helper.create_dataset_grpc_client().await;

    let reg = client
        .register_dataset(CAT, SCH, TBL, test_json_schema(), vec![])
        .await
        .unwrap();
    client
        .insert_batch(CAT, SCH, TBL, &reg.fingerprint, test_ipc_bytes())
        .await
        .unwrap();
    sleep(Duration::from_secs(3)).await;

    let sql = format!("SELECT score, label FROM {CAT}.{SCH}.{TBL}");
    let result = client.explain_query(&sql, false, 100).await.unwrap();
    let plan = result.logical_plan.unwrap();
    assert!(!plan.node_type.is_empty());
}

#[tokio::test]
async fn test_grpc_cancel_query() {
    let helper = setup_test().await;
    let mut client = helper.create_dataset_grpc_client().await;

    let result = client.cancel_query("ghost-id").await.unwrap();
    assert!(!result.cancelled);
}
