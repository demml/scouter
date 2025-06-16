use crate::common::{TestHelper, NAME, SPACE, VERSION};

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use http_body_util::BodyExt;
use scouter_dataframe::parquet::dataframe::ParquetDataFrame;
use scouter_drift::psi::PsiMonitor;
use scouter_drift::spc::SpcMonitor;
use scouter_server::api::archive::archive_old_data;
use scouter_types::contracts::DriftRequest;
use scouter_types::contracts::ProfileRequest;
use scouter_types::custom::CustomMetricAlertConfig;
use scouter_types::{
    custom::BinnedCustomMetrics,
    custom::{AlertThreshold, CustomDriftProfile, CustomMetric, CustomMetricDriftConfig},
    psi::{BinnedPsiFeatureMetrics, PsiAlertConfig, PsiDriftConfig},
    spc::{SpcAlertConfig, SpcDriftConfig, SpcDriftFeatures},
    DriftType, RecordType,
};
use sqlx::types::chrono::Utc;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_data_archive_spc() {
    let helper = TestHelper::new(false, false).await.unwrap();

    let (array, features) = helper.get_data();
    let alert_config = SpcAlertConfig::default();
    let config = SpcDriftConfig::new(
        Some(SPACE.to_string()),
        Some(NAME.to_string()),
        None,
        None,
        None,
        Some(alert_config),
        None,
    );

    let monitor = SpcMonitor::new();

    let profile = monitor
        .create_2d_drift_profile(&features, &array.view(), &config.unwrap())
        .unwrap();

    let request = ProfileRequest {
        space: profile.config.space.clone(),
        profile: profile.model_dump_json(),
        drift_type: DriftType::Spc,
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
    let long_term_records = helper.get_spc_drift_records(Some(10));

    // 0 day old records
    let short_term_records = helper.get_spc_drift_records(None);

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

    // Sleep for 2 seconds to allow the http consumer time to process all server records sent above.
    sleep(Duration::from_secs(2)).await;

    let record = archive_old_data(&helper.pool, &helper.config)
        .await
        .unwrap();

    assert!(record.spc);
    assert!(!record.psi);
    assert!(!record.custom);

    let df = ParquetDataFrame::new(&helper.config.storage_settings, &RecordType::Spc).unwrap();
    let path = format!("{SPACE}/{NAME}/{VERSION}/spc");

    let canonical_path = format!("{}/{}", df.storage_root(), path);
    let data_path = object_store::path::Path::from(canonical_path);
    let files = df.storage_client().list(Some(&data_path)).await.unwrap();

    assert!(!files.is_empty());

    let params = DriftRequest {
        space: SPACE.to_string(),
        name: NAME.to_string(),
        version: VERSION.to_string(),
        max_data_points: 100,
        drift_type: DriftType::Spc,
        begin_custom_datetime: Some(Utc::now() - chrono::Duration::days(15)),
        end_custom_datetime: Some(Utc::now()),
        ..Default::default()
    };

    let query_string = serde_qs::to_string(&params).unwrap();

    let request = Request::builder()
        .uri(format!("/scouter/drift/spc?{}", query_string))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;

    //assert response
    assert_eq!(response.status(), StatusCode::OK);
    let val = response.into_body().collect().await.unwrap().to_bytes();

    let results: SpcDriftFeatures = serde_json::from_slice(&val).unwrap();

    assert!(!results.features.is_empty());
    assert!(results.features["feature_1"].created_at.len() == 2);
    TestHelper::cleanup_storage()

    // query the data
}

#[tokio::test]
async fn test_data_archive_psi() {
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

    let config = PsiDriftConfig::new(SPACE, NAME, VERSION, alert_config, None, None);

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

    // Sleep for 5 second to allow the http consumer time to process all server records sent above.
    sleep(Duration::from_secs(5)).await;

    let record = archive_old_data(&helper.pool, &helper.config)
        .await
        .unwrap();

    assert!(!record.spc);
    assert!(record.psi);
    assert!(!record.custom);

    let df = ParquetDataFrame::new(&helper.config.storage_settings, &RecordType::Psi).unwrap();
    let path = format!("{SPACE}/{NAME}/{VERSION}/psi");

    let canonical_path = format!("{}/{}", df.storage_root(), path);
    let data_path = object_store::path::Path::from(canonical_path);
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
    TestHelper::cleanup_storage()
}

#[tokio::test]
async fn test_data_archive_custom() {
    let helper = TestHelper::new(false, false).await.unwrap();

    let alert_config = CustomMetricAlertConfig::default();
    let config =
        CustomMetricDriftConfig::new(SPACE, NAME, VERSION, 25, alert_config, None).unwrap();

    let alert_threshold = AlertThreshold::Above;
    let metric1 = CustomMetric::new("metric_1", 1.0, alert_threshold.clone(), None).unwrap();
    let metric2 = CustomMetric::new("metric_2", 1.0, alert_threshold, None).unwrap();
    let profile = CustomDriftProfile::new(config, vec![metric1, metric2], None).unwrap();

    let request = ProfileRequest {
        space: profile.config.space.clone(),
        profile: profile.model_dump_json(),
        drift_type: DriftType::Custom,
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
    let long_term_records = helper.get_custom_drift_records(Some(10));

    // 0 day old records
    let short_term_records = helper.get_custom_drift_records(None);

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

    // Sleep for 2 seconds to allow the http consumer time to process all server records sent above.
    sleep(Duration::from_secs(2)).await;

    let record = archive_old_data(&helper.pool, &helper.config)
        .await
        .unwrap();

    assert!(!record.spc);
    assert!(!record.psi);
    assert!(record.custom);

    let df = ParquetDataFrame::new(&helper.config.storage_settings, &RecordType::Custom).unwrap();
    let path = format!("{SPACE}/{NAME}/{VERSION}/custom");

    let canonical_path = format!("{}/{}", df.storage_root(), path);
    let data_path = object_store::path::Path::from(canonical_path);
    let files = df.storage_client().list(Some(&data_path)).await.unwrap();

    assert!(!files.is_empty());

    let params = DriftRequest {
        space: SPACE.to_string(),
        name: NAME.to_string(),
        version: VERSION.to_string(),
        max_data_points: 100,
        drift_type: DriftType::Custom,
        begin_custom_datetime: Some(Utc::now() - chrono::Duration::days(15)),
        end_custom_datetime: Some(Utc::now()),
        ..Default::default()
    };

    let query_string = serde_qs::to_string(&params).unwrap();

    let request = Request::builder()
        .uri(format!("/scouter/drift/custom?{}", query_string))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;

    //assert response
    assert_eq!(response.status(), StatusCode::OK);
    let val = response.into_body().collect().await.unwrap().to_bytes();

    let results: BinnedCustomMetrics = serde_json::from_slice(&val).unwrap();

    assert!(!results.metrics.is_empty());
    assert_eq!(results.metrics["metric_1"].created_at.len(), 2);
    TestHelper::cleanup_storage()
}
