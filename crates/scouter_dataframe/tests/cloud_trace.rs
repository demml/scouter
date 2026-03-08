//! GCS integration test for TraceSpanService.
//! Run via: make test.dataframe.cloud
//! Requires: SCOUTER_STORAGE_URI=gs://<bucket>/trace_test GOOGLE_ACCOUNT_JSON_BASE64=...

use chrono::Utc;
use object_store::path::Path as ObjPath;
use scouter_dataframe::{parquet::tracing::service::TraceSpanService, storage::ObjectStore};
use scouter_mocks::generate_trace_with_spans;
use scouter_settings::ObjectStorageSettings;
use scouter_types::sql::TraceSpan;

/// Derive the sub-path prefix from the storage URI (e.g. `gs://bucket/trace_test` → `trace_test`).
fn storage_prefix(settings: &ObjectStorageSettings) -> Option<String> {
    ["gs://", "s3://", "az://"]
        .iter()
        .find_map(|scheme| settings.storage_uri.strip_prefix(scheme))
        .and_then(|rest| rest.splitn(2, '/').nth(1))
        .map(|p| p.to_string())
}

/// Delete only the files under the test prefix — does not wipe the whole bucket.
async fn cleanup_remote(settings: &ObjectStorageSettings) {
    let store = ObjectStore::new(settings).unwrap();
    let prefix = storage_prefix(settings);
    let list_path = prefix.as_deref().map(ObjPath::from);
    if let Ok(files) = store.list(list_path.as_ref()).await {
        for file in files {
            let _ = store.delete(&ObjPath::from(file.as_str())).await;
        }
    }
}

#[tokio::test]
async fn test_trace_service_gcs_integration() {
    // Guard: only run when pointed at a GCS bucket.
    if !std::env::var("SCOUTER_STORAGE_URI")
        .unwrap_or_default()
        .starts_with("gs://")
    {
        eprintln!(
            "Skipping GCS trace integration test: \
             SCOUTER_STORAGE_URI not set to gs://"
        );
        return;
    }

    let storage_settings = ObjectStorageSettings::default();

    // Remove stale files from a previous failed run (idempotent).
    cleanup_remote(&storage_settings).await;

    let service = TraceSpanService::new(&storage_settings, 24, Some(2))
        .await
        .expect("Failed to initialize TraceSpanService on GCS");

    // Write a batch directly (bypasses the 2-second flush timer).
    let (_record, spans, _tags) = generate_trace_with_spans(5, 0);
    let first_trace_id = spans.first().unwrap().trace_id.clone();

    service
        .write_spans_direct(spans)
        .await
        .expect("Failed to write spans to GCS");

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
        .expect("Failed to query spans from GCS");

    assert!(
        !result_spans.is_empty(),
        "Expected ≥1 span from GCS but got 0. trace_id={:?}",
        first_trace_id
    );

    // All returned spans must fall within the query window.
    for span in &result_spans {
        assert!(
            span.start_time > start && span.start_time < end,
            "Span timestamp outside expected window: {:?}",
            span.start_time
        );
    }

    service.shutdown().await.expect("Shutdown failed");

    // Clean up — delete all Delta Lake files under the test prefix.
    cleanup_remote(&storage_settings).await;

    // Verify cleanup.
    let store = ObjectStore::new(&storage_settings).unwrap();
    let prefix = storage_prefix(&storage_settings);
    let list_path = prefix.as_deref().map(ObjPath::from);
    let remaining = store.list(list_path.as_ref()).await.unwrap_or_default();
    assert!(
        remaining.is_empty(),
        "Expected empty test prefix after cleanup but found {} file(s): {:?}",
        remaining.len(),
        remaining
    );
}
