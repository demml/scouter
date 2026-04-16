use crate::common::setup_test;
use arrow::array::{
    ArrayRef, BooleanArray, Date32Array, Float64Array, Int64Array, StringArray, StringViewArray,
    TimestampMicrosecondArray,
};
use arrow_array::RecordBatch;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::Datelike;
use http_body_util::BodyExt;
use scouter_dataframe::parquet::bifrost::ipc::batches_to_ipc_bytes;
use scouter_dataframe::{SERVICE_MAP_CATALOG, SERVICE_MAP_SCHEMA, SERVICE_MAP_TABLE};
use scouter_types::dataset::schema::{
    inject_system_columns, json_schema_to_arrow, SCOUTER_BATCH_ID, SCOUTER_CREATED_AT,
    SCOUTER_PARTITION_DATE,
};
use serde_json::Value;
use std::sync::Arc;
use tokio::time::{sleep, Duration};

const SERVICE_MAP_JSON_SCHEMA: &str = r#"{
    "type": "object",
    "properties": {
        "source_service": {"type": "string"},
        "destination_service": {"type": "string"},
        "endpoint": {"type": "string"},
        "method": {"type": "string"},
        "status_code": {"type": "integer"},
        "latency_ms": {"type": "number"},
        "timestamp": {"type": "string", "format": "date-time"},
        "trace_id": {"anyOf": [{"type": "string"}, {"type": "null"}]},
        "error": {"type": "boolean"},
        "source_verified": {"type": "boolean"},
        "tags": {"anyOf": [{"type": "string"}, {"type": "null"}]},
        "request_schema": {"anyOf": [{"type": "string"}, {"type": "null"}]}
    },
    "required": ["source_service", "destination_service", "endpoint", "method", "status_code", "latency_ms", "timestamp", "error", "source_verified"]
}"#;

/// (source, destination, endpoint, method, status_code, latency_ms, error)
type TestRecord<'a> = (&'a str, &'a str, &'a str, &'a str, i64, f64, bool);

fn service_map_ipc_bytes(records: &[TestRecord]) -> Vec<u8> {
    let user_schema = json_schema_to_arrow(SERVICE_MAP_JSON_SCHEMA).unwrap();
    let schema = Arc::new(inject_system_columns(user_schema).unwrap());

    let now = chrono::Utc::now();
    let epoch_days = now.date_naive().num_days_from_ce() - 719_163;
    let ts = now.timestamp_micros();
    let n = records.len();

    let columns: Vec<ArrayRef> = schema
        .fields()
        .iter()
        .map(|f| -> ArrayRef {
            match f.name().as_str() {
                "source_service" => Arc::new(StringViewArray::from(
                    records.iter().map(|r| r.0).collect::<Vec<_>>(),
                )),
                "destination_service" => Arc::new(StringViewArray::from(
                    records.iter().map(|r| r.1).collect::<Vec<_>>(),
                )),
                "endpoint" => Arc::new(StringViewArray::from(
                    records.iter().map(|r| r.2).collect::<Vec<_>>(),
                )),
                "method" => Arc::new(StringViewArray::from(
                    records.iter().map(|r| r.3).collect::<Vec<_>>(),
                )),
                "status_code" => Arc::new(Int64Array::from(
                    records.iter().map(|r| r.4).collect::<Vec<_>>(),
                )),
                "latency_ms" => Arc::new(Float64Array::from(
                    records.iter().map(|r| r.5).collect::<Vec<_>>(),
                )),
                "timestamp" => {
                    Arc::new(TimestampMicrosecondArray::from(vec![ts; n]).with_timezone("UTC"))
                }
                "trace_id" => Arc::new(StringViewArray::from(vec![None::<&str>; n])),
                "error" => Arc::new(BooleanArray::from(
                    records.iter().map(|r| r.6).collect::<Vec<_>>(),
                )),
                "source_verified" => Arc::new(BooleanArray::from(vec![false; n])),
                "tags" => Arc::new(StringViewArray::from(vec![None::<&str>; n])),
                "request_schema" => Arc::new(StringViewArray::from(vec![None::<&str>; n])),
                SCOUTER_CREATED_AT => {
                    Arc::new(TimestampMicrosecondArray::from(vec![ts; n]).with_timezone("UTC"))
                }
                SCOUTER_PARTITION_DATE => Arc::new(Date32Array::from(vec![epoch_days; n])),
                SCOUTER_BATCH_ID => Arc::new(StringArray::from(vec!["batch-1"; n])),
                other => panic!("Unexpected schema field: {other}"),
            }
        })
        .collect();

    let batch = RecordBatch::try_new(Arc::clone(&schema), columns).unwrap();
    batches_to_ipc_bytes(std::slice::from_ref(&batch)).unwrap()
}

async fn register_and_insert(helper: &crate::common::TestHelper, records: &[TestRecord<'_>]) {
    let mut client = helper.create_dataset_grpc_client().await;
    let reg = client
        .register_dataset(
            SERVICE_MAP_CATALOG,
            SERVICE_MAP_SCHEMA,
            SERVICE_MAP_TABLE,
            SERVICE_MAP_JSON_SCHEMA,
            vec![],
        )
        .await
        .unwrap();
    client
        .insert_batch(
            SERVICE_MAP_CATALOG,
            SERVICE_MAP_SCHEMA,
            SERVICE_MAP_TABLE,
            &reg.fingerprint,
            service_map_ipc_bytes(records),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn test_get_service_graph_invalid_since() {
    let helper = setup_test().await;

    let request = Request::builder()
        .uri("/scouter/service/graph?since=not-a-date")
        .body(Body::empty())
        .unwrap();
    let resp = helper.send_oneshot(request).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_get_service_graph_full() {
    let helper = setup_test().await;

    let records: &[TestRecord] = &[
        ("svc-a", "svc-b", "/health", "GET", 200, 10.0, false),
        ("svc-a", "svc-b", "/health", "GET", 200, 15.0, false),
        ("svc-b", "svc-c", "/predict", "POST", 200, 50.0, false),
    ];
    register_and_insert(&helper, records).await;
    sleep(Duration::from_secs(3)).await;

    let request = Request::builder()
        .uri("/scouter/service/graph")
        .body(Body::empty())
        .unwrap();
    let resp = helper.send_oneshot(request).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&body).unwrap();
    let edges = v["edges"].as_array().unwrap();
    assert!(!edges.is_empty(), "expected at least one edge");

    let edge = &edges[0];
    assert!(
        edge["source_service"].as_str().is_some(),
        "source_service missing"
    );
    assert!(
        edge["destination_service"].as_str().is_some(),
        "destination_service missing"
    );
    assert!(edge["endpoint"].as_str().is_some(), "endpoint missing");
    assert!(
        edge["total_calls"].as_i64().is_some(),
        "total_calls missing"
    );
    assert!(
        edge["avg_latency_ms"].as_f64().is_some(),
        "avg_latency_ms missing"
    );
}

#[tokio::test]
async fn test_get_service_graph_filtered() {
    let helper = setup_test().await;

    let records: &[TestRecord] = &[
        ("svc-a", "svc-b", "/health", "GET", 200, 10.0, false),
        ("svc-a", "svc-c", "/health", "GET", 200, 12.0, false),
        ("svc-b", "svc-c", "/predict", "POST", 200, 50.0, false),
    ];
    register_and_insert(&helper, records).await;
    sleep(Duration::from_secs(3)).await;

    let request = Request::builder()
        .uri("/scouter/service/graph?service_name=svc-c")
        .body(Body::empty())
        .unwrap();
    let resp = helper.send_oneshot(request).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let v: Value = serde_json::from_slice(&body).unwrap();
    let edges = v["edges"].as_array().unwrap();
    assert!(!edges.is_empty(), "expected edges for svc-c");

    for edge in edges {
        assert_eq!(
            edge["destination_service"].as_str().unwrap(),
            "svc-c",
            "filter should exclude non-svc-c destinations"
        );
    }
}
