use std::time::Duration;
// storage integration tests for cloud storage
use crate::common::{TestHelper, NAME, SPACE, VERSION};

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use http_body_util::BodyExt;
use scouter_contracts::DriftRequest;
use scouter_contracts::ProfileRequest;
use scouter_dataframe::parquet::dataframe::ParquetDataFrame;
use scouter_drift::psi::PsiMonitor;
use scouter_server::api::archive::archive_old_data;
use scouter_types::{
    psi::{BinnedPsiFeatureMetrics, PsiAlertConfig, PsiDriftConfig},
    DriftType, RecordType,
};
use sqlx::types::chrono::Utc;
use tokio::time::sleep;

#[tokio::test]
async fn test_storage_integration_cloud() {
    let helper = TestHelper::new(false, false).await.unwrap();

    // create profile
    let (array, features) = helper.get_data();
    let alert_config = PsiAlertConfig {
        features_to_monitor: vec![
            "feature_1".to_string(),
            "feature_2".to_string(),
            "feature_3".to_string(),
        ],
        ..Default::default()
    };

    let config = PsiDriftConfig::new(SPACE, NAME, VERSION, alert_config, None);

    let monitor = PsiMonitor::new();

    let profile = monitor
        .create_2d_drift_profile(&features, &array.view(), &config.unwrap())
        .unwrap();

    let request = ProfileRequest {
        space: profile.config.space.clone(),
        profile: profile.model_dump_json(),
        drift_type: DriftType::Psi,
    };

    let body = serde_json::to_string(&request).unwrap();

    let request = Request::builder()
        .uri("/scouter/profile")
        .method("POST")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap();

    let response = helper.send_oneshot(request).await;

    //assert response
    assert_eq!(response.status(), StatusCode::OK);

    // 10 day old records
    let long_term_records = helper.get_psi_drift_records(Some(10));

    // 0 day old records
    let short_term_records = helper.get_psi_drift_records(None);

    for records in [short_term_records, long_term_records].iter() {
        let body = serde_json::to_string(records).unwrap();
        let request = Request::builder()
            .uri("/scouter/drift")
            .method("POST")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = helper.send_oneshot(request).await;

        //assert response
        assert_eq!(response.status(), StatusCode::OK);
    }

    // Sleep for 3 second to allow the http consumer time to process all server records sent above.
    sleep(Duration::from_secs(3)).await;

    let record = archive_old_data(&helper.pool, &helper.config)
        .await
        .unwrap();
    //
    assert!(!record.spc);
    assert!(record.psi);
    assert!(!record.custom);
    //
    let df = ParquetDataFrame::new(&helper.config.storage_settings, &RecordType::Psi).unwrap();
    let path = format!("{SPACE}/{NAME}/{VERSION}/psi");

    let data_path = object_store::path::Path::from(path);
    let files = df.storage_client().list(Some(&data_path)).await.unwrap();

    assert!(!files.is_empty());

    let params = DriftRequest {
        space: SPACE.to_string(),
        name: NAME.to_string(),
        version: VERSION.to_string(),
        max_data_points: 100,
        drift_type: DriftType::Psi,
        begin_custom_datetime: Some(Utc::now() - chrono::Duration::days(15)),
        end_custom_datetime: Some(Utc::now()),
        ..Default::default()
    };

    let query_string = serde_qs::to_string(&params).unwrap();

    let request = Request::builder()
        .uri(format!("/scouter/drift/psi?{}", query_string))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;

    //assert response
    assert_eq!(response.status(), StatusCode::OK);
    let val = response.into_body().collect().await.unwrap().to_bytes();
    let results: BinnedPsiFeatureMetrics = serde_json::from_slice(&val).unwrap();

    assert!(!results.features.is_empty());

    for file in files.iter() {
        let file_path = object_store::path::Path::from(file.to_string());
        df.storage_client().delete(&file_path).await.unwrap();
    }

    // assert that the data is deleted
    let files = df.storage_client().list(Some(&data_path)).await.unwrap();

    assert!(files.is_empty());

    TestHelper::cleanup_storage()
}
