use crate::common::{TestHelper, NAME, SPACE, VERSION};

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use http_body_util::BodyExt;
use potato_head::{create_score_prompt, LLMTestServer};
use scouter_dataframe::parquet::dataframe::ParquetDataFrame;
use scouter_drift::psi::PsiMonitor;
use scouter_drift::spc::SpcMonitor;
use scouter_server::api::archive::archive_old_data;
use scouter_types::contracts::DriftRequest;
use scouter_types::custom::CustomMetricAlertConfig;
use scouter_types::llm::{LLMAlertConfig, LLMDriftConfig, LLMDriftMetric, LLMDriftProfile};
use scouter_types::{
    custom::{CustomDriftProfile, CustomMetric, CustomMetricDriftConfig},
    psi::{BinnedPsiFeatureMetrics, PsiAlertConfig, PsiDriftConfig},
    spc::{SpcAlertConfig, SpcDriftConfig, SpcDriftFeatures},
    AlertThreshold, BinnedMetrics, RecordType,
};
use sqlx::types::chrono::Utc;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_data_archive_spc() {
    let helper = TestHelper::new(false, false).await.unwrap();

    let (array, features) = helper.get_data();
    let alert_config = SpcAlertConfig::default();
    let config = SpcDriftConfig::new(SPACE, NAME, VERSION, None, None, Some(alert_config), None);

    let monitor = SpcMonitor::new();

    let mut profile = monitor
        .create_2d_drift_profile(&features, &array.view(), &config.unwrap())
        .unwrap();

    let uid = helper
        .register_drift_profile(profile.create_profile_request().unwrap())
        .await;

    profile.config.uid = uid.clone();

    // 10 day old records
    let long_term_records = helper.get_spc_drift_records(Some(10), &profile.config.uid);

    // 0 day old records
    let short_term_records = helper.get_spc_drift_records(None, &profile.config.uid);

    for records in [short_term_records, long_term_records].iter() {
        let body = serde_json::to_string(records).unwrap();
        let request = Request::builder()
            .uri("/scouter/message")
            .method("POST")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = helper.send_oneshot(request).await;

        //assert response
        assert_eq!(response.status(), StatusCode::OK);
    }

    // Sleep for 5 seconds to allow the http consumer time to process all server records sent above.
    sleep(Duration::from_secs(5)).await;

    let record = archive_old_data(&helper.pool, &helper.config)
        .await
        .unwrap();

    assert!(record.spc);
    assert!(!record.psi);
    assert!(!record.custom);

    let df = ParquetDataFrame::new(&helper.config.storage_settings, &RecordType::Spc).unwrap();
    let path = format!("{}/spc", profile.config.uid);

    let canonical_path = format!("{}/{}", df.storage_root(), path);
    let data_path = object_store::path::Path::from(canonical_path);
    let files = df.storage_client().list(Some(&data_path)).await.unwrap();

    assert!(!files.is_empty());

    let params = DriftRequest {
        space: SPACE.to_string(),
        uid: profile.config.uid.clone(),
        max_data_points: 100,
        begin_custom_datetime: Some(Utc::now() - chrono::Duration::days(15)),
        end_custom_datetime: Some(Utc::now()),
        ..Default::default()
    };

    let query_string = serde_qs::to_string(&params).unwrap();

    let request = Request::builder()
        .uri(format!("/scouter/drift/spc?{query_string}"))
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

    let config = PsiDriftConfig {
        space: SPACE.to_string(),
        name: NAME.to_string(),
        version: VERSION.to_string(),
        alert_config,
        ..Default::default()
    };

    let monitor = PsiMonitor::new();

    let mut profile = monitor
        .create_2d_drift_profile(&features, &array.view(), &config)
        .unwrap();

    let uid = helper
        .register_drift_profile(profile.create_profile_request().unwrap())
        .await;

    profile.config.uid = uid.clone();

    // 10 day old records
    let long_term_records = helper.get_psi_drift_records(Some(10), &profile.config.uid);

    // 0 day old records
    let short_term_records = helper.get_psi_drift_records(None, &profile.config.uid);

    for records in [short_term_records, long_term_records].iter() {
        let body = serde_json::to_string(records).unwrap();
        let request = Request::builder()
            .uri("/scouter/message")
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
    let path = format!("{}/psi", profile.config.uid);

    let canonical_path = format!("{}/{}", df.storage_root(), path);
    let data_path = object_store::path::Path::from(canonical_path);
    let files = df.storage_client().list(Some(&data_path)).await.unwrap();

    assert!(!files.is_empty());

    let params = DriftRequest {
        space: SPACE.to_string(),
        uid: profile.config.uid.clone(),
        max_data_points: 100,
        begin_custom_datetime: Some(Utc::now() - chrono::Duration::days(15)),
        end_custom_datetime: Some(Utc::now()),
        ..Default::default()
    };

    let query_string = serde_qs::to_string(&params).unwrap();

    let request = Request::builder()
        .uri(format!("/scouter/drift/psi?{query_string}"))
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
    let mut profile = CustomDriftProfile::new(config, vec![metric1, metric2]).unwrap();

    let uid = helper
        .register_drift_profile(profile.create_profile_request().unwrap())
        .await;

    profile.config.uid = uid.clone();

    // 20 day old records
    let long_term_records = helper.get_custom_drift_records(Some(20), &profile.config.uid);

    // 10 day old records
    let medium_term_records = helper.get_custom_drift_records(Some(10), &profile.config.uid);

    // 0 day old records
    let short_term_records = helper.get_custom_drift_records(None, &profile.config.uid);

    for records in [short_term_records, medium_term_records, long_term_records].iter() {
        let body = serde_json::to_string(records).unwrap();
        let request = Request::builder()
            .uri("/scouter/message")
            .method("POST")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = helper.send_oneshot(request).await;

        //assert response
        assert_eq!(response.status(), StatusCode::OK);
    }

    // Sleep for 5 seconds to allow the http consumer time to process all server records sent above.
    sleep(Duration::from_secs(5)).await;

    let record = archive_old_data(&helper.pool, &helper.config)
        .await
        .unwrap();

    assert!(!record.spc);
    assert!(!record.psi);
    assert!(record.custom);

    let df = ParquetDataFrame::new(&helper.config.storage_settings, &RecordType::Custom).unwrap();
    let path = format!("{}/custom", profile.config.uid);

    let canonical_path = format!("{}/{}", df.storage_root(), path);
    let data_path = object_store::path::Path::from(canonical_path);
    let files = df.storage_client().list(Some(&data_path)).await.unwrap();

    assert!(!files.is_empty());

    let params = DriftRequest {
        space: SPACE.to_string(),
        uid: profile.config.uid.clone(),
        max_data_points: 100,
        begin_custom_datetime: Some(Utc::now() - chrono::Duration::days(15)),
        end_custom_datetime: Some(Utc::now()),
        ..Default::default()
    };

    let query_string = serde_qs::to_string(&params).unwrap();

    let request = Request::builder()
        .uri(format!("/scouter/drift/custom?{query_string}"))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = helper.send_oneshot(request).await;

    //assert response
    assert_eq!(response.status(), StatusCode::OK);
    let val = response.into_body().collect().await.unwrap().to_bytes();

    let results: BinnedMetrics = serde_json::from_slice(&val).unwrap();

    assert!(!results.metrics.is_empty());
    assert_eq!(results.metrics["metric_0"].created_at.len(), 2);
    TestHelper::cleanup_storage()
}

#[test]
fn test_data_archive_llm_drift_record() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut mock = LLMTestServer::new();
    mock.start_server().unwrap();

    let helper = runtime.block_on(async { TestHelper::new(false, false).await.unwrap() });

    let alert_config = LLMAlertConfig::default();
    let config = LLMDriftConfig::new(SPACE, NAME, VERSION, 25, alert_config, None).unwrap();
    let prompt = create_score_prompt(Some(vec!["input".to_string()]));

    let _alert_threshold = AlertThreshold::Above;
    let metric1 = LLMDriftMetric::new(
        "metric1",
        5.0,
        AlertThreshold::Above,
        None,
        Some(prompt.clone()),
    )
    .unwrap();
    let metric2 = LLMDriftMetric::new(
        "metric2",
        3.0,
        AlertThreshold::Below,
        Some(1.0),
        Some(prompt.clone()),
    )
    .unwrap();
    let llm_metrics = vec![metric1, metric2];
    let mut profile = runtime
        .block_on(async { LLMDriftProfile::from_metrics(config, llm_metrics).await })
        .unwrap();

    let uid = runtime.block_on(async {
        helper
            .register_drift_profile(profile.create_profile_request().unwrap())
            .await
    });

    profile.config.uid = uid.clone();

    // 10 day old records
    let long_term_records = helper.get_llm_drift_records(Some(10), &profile.config.uid);

    // 0 day old records
    let short_term_records = helper.get_llm_drift_records(None, &profile.config.uid);

    for records in [short_term_records, long_term_records].iter() {
        let body = serde_json::to_string(records).unwrap();
        let request = Request::builder()
            .uri("/scouter/message")
            .method("POST")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = runtime.block_on(async { helper.send_oneshot(request).await });

        //assert response
        assert_eq!(response.status(), StatusCode::OK);
    }

    // Sleep for 5 seconds to allow the http consumer time to process all server records sent above.
    runtime.block_on(async { sleep(Duration::from_secs(5)).await });

    let record = runtime.block_on(async {
        archive_old_data(&helper.pool, &helper.config)
            .await
            .unwrap()
    });

    assert!(!record.spc);
    assert!(!record.psi);
    assert!(!record.custom);
    assert!(!record.llm_metric);
    assert!(record.llm_drift);

    let df = ParquetDataFrame::new(&helper.config.storage_settings, &RecordType::LLMDrift).unwrap();
    let path = format!("{}/llm_drift", profile.config.uid);

    let canonical_path = format!("{}/{}", df.storage_root(), path);
    let data_path = object_store::path::Path::from(canonical_path);

    let files =
        runtime.block_on(async { df.storage_client().list(Some(&data_path)).await.unwrap() });

    assert!(!files.is_empty());

    mock.stop_server().unwrap();
    TestHelper::cleanup_storage()
}

#[test]
fn test_data_archive_llm_drift_metrics() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let mut mock = LLMTestServer::new();
    mock.start_server().unwrap();

    let helper = runtime.block_on(async { TestHelper::new(false, false).await.unwrap() });

    let alert_config = LLMAlertConfig::default();
    let config = LLMDriftConfig::new(SPACE, NAME, VERSION, 25, alert_config, None).unwrap();
    let prompt = create_score_prompt(Some(vec!["input".to_string()]));

    let _alert_threshold = AlertThreshold::Above;
    let metric1 = LLMDriftMetric::new(
        "metric1",
        5.0,
        AlertThreshold::Above,
        None,
        Some(prompt.clone()),
    )
    .unwrap();
    let metric2 = LLMDriftMetric::new(
        "metric2",
        3.0,
        AlertThreshold::Below,
        Some(1.0),
        Some(prompt.clone()),
    )
    .unwrap();
    let llm_metrics = vec![metric1, metric2];
    let mut profile = runtime
        .block_on(async { LLMDriftProfile::from_metrics(config, llm_metrics).await })
        .unwrap();

    let uid = runtime.block_on(async {
        helper
            .register_drift_profile(profile.create_profile_request().unwrap())
            .await
    });

    profile.config.uid = uid.clone();

    // 20 day old records
    let long_term_records = helper.get_llm_drift_metrics(Some(20), &profile.config.uid);

    // 10 day old records
    let mid_term_records = helper.get_llm_drift_metrics(Some(10), &profile.config.uid);

    // 0 day old records
    let short_term_records = helper.get_llm_drift_metrics(None, &profile.config.uid);

    for records in [short_term_records, long_term_records, mid_term_records].iter() {
        let body = serde_json::to_string(records).unwrap();
        let request = Request::builder()
            .uri("/scouter/message")
            .method("POST")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = runtime.block_on(async { helper.send_oneshot(request).await });

        //assert response
        assert_eq!(response.status(), StatusCode::OK);
    }

    // Sleep for 5 seconds to allow the http consumer time to process all server records sent above.
    runtime.block_on(async { sleep(Duration::from_secs(5)).await });
    let record = runtime.block_on(async {
        archive_old_data(&helper.pool, &helper.config)
            .await
            .unwrap()
    });

    assert!(!record.spc);
    assert!(!record.psi);
    assert!(!record.custom);
    assert!(record.llm_metric);
    assert!(!record.llm_drift);

    let df =
        ParquetDataFrame::new(&helper.config.storage_settings, &RecordType::LLMMetric).unwrap();
    let path = format!("{}/llm_metric", profile.config.uid);

    let canonical_path = format!("{}/{}", df.storage_root(), path);
    let data_path = object_store::path::Path::from(canonical_path);

    let files =
        runtime.block_on(async { df.storage_client().list(Some(&data_path)).await.unwrap() });

    assert!(!files.is_empty());

    let params = DriftRequest {
        space: SPACE.to_string(),
        uid: profile.config.uid.clone(),
        max_data_points: 100,
        begin_custom_datetime: Some(Utc::now() - chrono::Duration::days(30)),
        end_custom_datetime: Some(Utc::now()),
        ..Default::default()
    };

    let query_string = serde_qs::to_string(&params).unwrap();

    let request = Request::builder()
        .uri(format!("/scouter/drift/llm?{query_string}"))
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let response = runtime.block_on(async { helper.send_oneshot(request).await });

    //assert response
    assert_eq!(response.status(), StatusCode::OK);
    let val = runtime.block_on(async { response.into_body().collect().await.unwrap().to_bytes() });

    let results: BinnedMetrics = serde_json::from_slice(&val).unwrap();

    assert!(!results.metrics.is_empty());
    assert_eq!(results.metrics["metric0"].created_at.len(), 3);

    mock.stop_server().unwrap();
    TestHelper::cleanup_storage();
}
