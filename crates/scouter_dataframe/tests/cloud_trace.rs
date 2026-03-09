//! Cloud storage integration tests for TraceSpanService.
//!
//! A single shared test body covers GCS, S3, and Azure. Three thin wrappers
//! guard on their respective URI scheme so each can be targeted independently
//! in CI (e.g. `cargo test test_trace_service_s3`).
//!
//! Run via: `make test.dataframe.cloud`
//!
//! Required env vars — set whichever matches the target backend:
//!
//! | Backend | `SCOUTER_STORAGE_URI`          | Auth env vars                                              |
//! |---------|--------------------------------|------------------------------------------------------------|
//! | GCS     | `gs://<bucket>/trace_test`     | `GOOGLE_ACCOUNT_JSON_BASE64` or ADC                        |
//! | S3      | `s3://<bucket>/trace_test`     | `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_REGION` |
//! | Azure   | `az://<container>/trace_test`  | `AZURE_STORAGE_ACCOUNT_NAME`, `AZURE_STORAGE_ACCOUNT_KEY`  |

use chrono::Utc;
use object_store::path::Path as ObjPath;
use scouter_dataframe::{parquet::tracing::service::TraceSpanService, storage::ObjectStore};
use scouter_mocks::generate_trace_with_spans;
use scouter_settings::ObjectStorageSettings;
use scouter_types::sql::TraceSpan;

// ── helpers ───────────────────────────────────────────────────────────────────

const CLOUD_SCHEMES: &[&str] = &["gs://", "s3://", "az://"];

/// Return the sub-path prefix for the test URI so cleanup only touches the
/// test directory, not the whole bucket/container.
///
/// e.g. `gs://my-bucket/trace_test` → `Some("trace_test")`
fn storage_prefix(settings: &ObjectStorageSettings) -> Option<String> {
    CLOUD_SCHEMES
        .iter()
        .find_map(|scheme| settings.storage_uri.strip_prefix(scheme))
        .and_then(|rest| rest.split_once('/').map(|(_, path)| path.to_string()))
}

/// Delete every object under the test prefix. Errors are silently ignored so
/// a partial cleanup failure does not block the test.
async fn cleanup_remote(settings: &ObjectStorageSettings) {
    let store = match ObjectStore::new(settings) {
        Ok(s) => s,
        Err(_) => return,
    };
    let prefix = storage_prefix(settings);
    let list_path = prefix.as_deref().map(ObjPath::from);
    if let Ok(files) = store.list(list_path.as_ref()).await {
        for file in files {
            let _ = store.delete(&ObjPath::from(file.as_str())).await;
        }
    }
}

/// Core integration test body — provider-agnostic.
///
/// `label` is included in assertion messages to identify which backend failed.
async fn run_cloud_integration_test(settings: &ObjectStorageSettings, label: &str) {
    // Remove stale files from a previous failed run (idempotent).
    cleanup_remote(settings).await;

    let service = TraceSpanService::new(settings, 24, Some(2))
        .await
        .unwrap_or_else(|e| panic!("Failed to initialize TraceSpanService on {label}: {e}"));

    // Write a batch directly (bypasses the flush timer).
    let (_record, spans, _tags) = generate_trace_with_spans(5, 0);
    let first_trace_id = spans.first().unwrap().trace_id.clone();

    service
        .write_spans_direct(spans)
        .await
        .unwrap_or_else(|e| panic!("Failed to write spans to {label}: {e}"));

    // Query back via the DataFusion layer.
    let start = Utc::now() - chrono::Duration::hours(1);
    let end = Utc::now() + chrono::Duration::hours(1);

    let result_spans: Vec<TraceSpan> = service
        .query_service
        .get_trace_spans(
            Some(first_trace_id.as_bytes()),
            None,
            Some(&start),
            Some(&end),
            None,
        )
        .await
        .unwrap_or_else(|e| panic!("Failed to query spans from {label}: {e}"));

    assert!(
        !result_spans.is_empty(),
        "[{label}] Expected ≥1 span but got 0. trace_id={:?}",
        first_trace_id
    );

    for span in &result_spans {
        assert!(
            span.start_time > start && span.start_time < end,
            "[{label}] Span timestamp outside query window: {:?}",
            span.start_time
        );
    }

    service
        .shutdown()
        .await
        .unwrap_or_else(|e| panic!("[{label}] Shutdown failed: {e}"));

    // Clean up and verify the prefix is empty.
    cleanup_remote(settings).await;

    let store = ObjectStore::new(settings).unwrap();
    let prefix = storage_prefix(settings);
    let list_path = prefix.as_deref().map(ObjPath::from);
    let remaining = store.list(list_path.as_ref()).await.unwrap_or_default();
    assert!(
        remaining.is_empty(),
        "[{label}] Expected empty prefix after cleanup but found {} file(s): {:?}",
        remaining.len(),
        remaining
    );
}

// ── per-provider test wrappers ────────────────────────────────────────────────

#[tokio::test]
async fn test_trace_service_gcs_integration() {
    if !std::env::var("SCOUTER_STORAGE_URI")
        .unwrap_or_default()
        .starts_with("gs://")
    {
        eprintln!("Skipping GCS test: SCOUTER_STORAGE_URI not set to gs://");
        return;
    }
    run_cloud_integration_test(&ObjectStorageSettings::default(), "GCS").await;
}

#[tokio::test]
async fn test_trace_service_s3_integration() {
    if !std::env::var("SCOUTER_STORAGE_URI")
        .unwrap_or_default()
        .starts_with("s3://")
    {
        eprintln!("Skipping S3 test: SCOUTER_STORAGE_URI not set to s3://");
        return;
    }
    run_cloud_integration_test(&ObjectStorageSettings::default(), "S3").await;
}

#[tokio::test]
async fn test_trace_service_azure_integration() {
    if !std::env::var("SCOUTER_STORAGE_URI")
        .unwrap_or_default()
        .starts_with("az://")
    {
        eprintln!("Skipping Azure test: SCOUTER_STORAGE_URI not set to az://");
        return;
    }
    run_cloud_integration_test(&ObjectStorageSettings::default(), "Azure").await;
}
