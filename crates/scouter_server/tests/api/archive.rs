use crate::common::{TestHelper, NAME, SPACE, VERSION};

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use scouter_dataframe::parquet::dataframe::ParquetDataFrame;
use scouter_dataframe::parquet::spc::dataframe_to_spc_drift_features;
use scouter_server::api::data_manager::archive_old_data;
use scouter_settings::ObjectStorageSettings;
use scouter_types::RecordType;
use sqlx::types::chrono::Utc;

#[tokio::test]
async fn test_data_archive_spc() {
    let helper = TestHelper::new(false, false).await.unwrap();
    let records = helper.get_spc_drift_records();
    let body = serde_json::to_string(&records).unwrap();
    let start_utc = Utc::now();
    let request = Request::builder()
        .uri("/scouter/drift")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = helper.send_oneshot(request).await;

    //assert response
    assert_eq!(response.status(), StatusCode::OK);

    let db_client = helper.get_db_client().await;
    let storage_settings = ObjectStorageSettings::default();
    let retention_period = 0; // days
    archive_old_data(&db_client, &storage_settings, &retention_period)
        .await
        .unwrap();

    let df = ParquetDataFrame::new(&storage_settings, &RecordType::Spc).unwrap();
    let path = "space/name/1.0.0/spc";

    let canonical_path = format!("{}/{}", df.storage_root(), path);
    let data_path = object_store::path::Path::from(canonical_path);
    let files = df.storage_client().list(Some(&data_path)).await.unwrap();

    assert!(!files.is_empty());

    let read_df = df
        .get_binned_metrics(
            path,
            &0.01,
            &start_utc,
            &Utc::now(),
            "space",
            "name",
            "1.0.0",
        )
        .await
        .unwrap();

    // dataframe to spc
    dataframe_to_spc_drift_features(read_df).await.unwrap();
}

#[tokio::test]
async fn test_data_archive_psi() {
    let helper = TestHelper::new(false, false).await.unwrap();
    let records = helper.get_psi_drift_records();
    let body = serde_json::to_string(&records).unwrap();
    let start_utc = Utc::now();
    let request = Request::builder()
        .uri("/scouter/drift")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = helper.send_oneshot(request).await;

    //assert response
    assert_eq!(response.status(), StatusCode::OK);

    let db_client = helper.get_db_client().await;
    let storage_settings = ObjectStorageSettings::default();
    let retention_period = 0; // days
    archive_old_data(&db_client, &storage_settings, &retention_period)
        .await
        .unwrap();

    let df = ParquetDataFrame::new(&storage_settings, &RecordType::Spc).unwrap();
    let path = format!("{SPACE}/{NAME}/{VERSION}/spc");

    let canonical_path = format!("{}/{}", df.storage_root(), path);
    let data_path = object_store::path::Path::from(canonical_path);
    let files = df.storage_client().list(Some(&data_path)).await.unwrap();

    assert!(!files.is_empty());

    let read_df = df
        .get_binned_metrics(
            &path,
            &0.01,
            &start_utc,
            &Utc::now(),
            "space",
            "name",
            "1.0.0",
        )
        .await
        .unwrap();

    // dataframe to spc
    dataframe_to_spc_drift_features(read_df).await.unwrap();
}
