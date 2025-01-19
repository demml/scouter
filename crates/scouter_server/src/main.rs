pub mod api;

use crate::api::middleware::metrics::metrics_app;
use crate::api::setup::setup_logging;
use crate::api::state::AppState;
use anyhow::Context;
use api::router::create_router;
use axum::Router;
use scouter_drift::DriftExecutor;
use scouter_settings::ScouterServerConfig;
use scouter_sql::PostgresClient;
use std::sync::Arc;
use tracing::{error, info};

#[cfg(feature = "kafka")]
use scouter_events::consumer::kafka::startup_kafka;

#[cfg(feature = "rabbitmq")]
use scouter_events::consumer::rabbitmq::startup_rabbitmq;

/// Start the metrics server for prometheus
async fn start_metrics_server() -> Result<(), anyhow::Error> {
    let app = metrics_app().with_context(|| "Failed to setup metrics app")?;

    // NOTE: expose metrics endpoint on a different port
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8001")
        .await
        .with_context(|| "Failed to bind to port 8001 for metrics server")?;
    axum::serve(listener, app)
        .await
        .with_context(|| "Failed to start metrics server")?;

    Ok(())
}

/// Setup drift polling workers
///
/// This function will start a number of drift polling workers based on the number of workers
///
/// # Arguments
///
/// * `config` - The server configuration
///
async fn setup_polling_workers(config: &ScouterServerConfig) -> Result<(), anyhow::Error> {
    for i in 0..config.polling_settings.num_workers {
        info!("Starting drift schedule poller: {}", i);

        let db_settings = config.database_settings.clone();
        let db_client = PostgresClient::new(None, Some(&db_settings))
            .await
            .with_context(|| "Failed to create Postgres client")?;

        tokio::spawn(async move {
            let mut drift_executor = DriftExecutor::new(db_client);
            loop {
                if let Err(e) = drift_executor.poll_for_tasks().await {
                    error!("Alert poller error: {:?}", e);
                }
            }
        });
    }

    Ok(())
}

/// Create the main server
///
/// This function will create the main server with the given configuration
///
/// # Arguments
///
/// * `config` - The server configuration
///
/// # Returns
///
/// The main server router
async fn create_app(config: ScouterServerConfig) -> Result<Router, anyhow::Error> {
    // setup logging, soft fail if it fails
    let _ = setup_logging().await.is_ok();

    // db for app state and kafka
    // start server
    let db_client = PostgresClient::new(None, Some(&config.database_settings))
        .await
        .with_context(|| "Failed to create Postgres client")?;

    // setup background kafka task if kafka is enabled
    #[cfg(feature = "kafka")]
    if config.kafka_enabled() {
        startup_kafka(&db_client.pool, &config).await?;
    }

    // setup background rabbitmq task if rabbitmq is enabled
    #[cfg(feature = "rabbitmq")]
    if config.rabbitmq_enabled() {
        startup_rabbitmq(&db_client.pool, &config).await?;
    }

    // ##################### run drift polling background tasks #####################
    setup_polling_workers(&config).await?;

    let router = create_router(Arc::new(AppState { db: db_client }))
        .await
        .with_context(|| "Failed to create router")?;

    Ok(router)
}

/// Start the main server
async fn start_main_server() -> Result<(), anyhow::Error> {
    let config = ScouterServerConfig::default();
    let addr = format!("0.0.0.0:{}", config.server_port.clone());
    let listener = tokio::net::TcpListener::bind(addr.clone())
        .await
        .with_context(|| "Failed to bind to port 8000")?;

    let router = create_app(config).await?;

    info!(
        "🚀 Scouter Server started successfully on {:?}",
        addr.clone().to_string()
    );
    axum::serve(listener, router)
        .await
        .with_context(|| "Failed to start main server")?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let (_main_server, _metrics_server) = tokio::join!(start_main_server(), start_metrics_server());
    Ok(())
}

#[cfg(test)]
mod tests {

    use super::*;
    use axum::response::Response;
    use axum::{
        body::Body,
        http::{header, Request, StatusCode},
        Router,
    };
    use http_body_util::BodyExt;
    use rand::Rng;
    use scouter_contracts::{
        DriftRequest, GetProfileRequest, ProfileRequest, ProfileStatusRequest,
    };
    use scouter_drift::psi::PsiMonitor;
    use scouter_sql::sql::schema::{BinnedCustomMetrics, SpcFeatureResult};
    use scouter_types::custom::{
        AlertThreshold, CustomDriftProfile, CustomMetric, CustomMetricAlertConfig,
        CustomMetricDriftConfig,
    };
    use scouter_types::psi::BinnedPsiFeatureMetrics;
    use scouter_types::psi::{PsiAlertConfig, PsiDriftConfig, PsiDriftViz};
    use scouter_types::{CustomMetricServerRecord, PsiServerRecord};
    use scouter_types::{
        DriftType, RecordType, ServerRecord, ServerRecords, SpcServerRecord, TimeInterval,
    };
    // for `collect`
    use crate::api::routes::health::Alive;
    use ndarray::Array;
    use ndarray_rand::rand_distr::Uniform;
    use ndarray_rand::RandomExt;
    use scouter_drift::spc::monitor::SpcMonitor;
    use scouter_types::spc::{SpcAlertConfig, SpcDriftConfig};
    use sqlx::{Pool, Postgres};
    use tower::util::ServiceExt;

    pub async fn cleanup(pool: &Pool<Postgres>) -> Result<(), anyhow::Error> {
        sqlx::raw_sql(
            r#"
            DELETE 
            FROM drift;

            DELETE 
            FROM observability_metrics;

            DELETE
            FROM custom_metrics;

            DELETE
            FROM drift_alerts;

            DELETE
            FROM drift_profile;

            DELETE
            FROM observed_bin_count;
            "#,
        )
        .fetch_all(pool)
        .await
        .unwrap();

        Ok(())
    }

    pub struct TestHelper {
        app: Router,
    }

    impl TestHelper {
        pub async fn new(enable_kafka: bool, enable_rabbitmq: bool) -> Result<Self, anyhow::Error> {
            if enable_kafka {
                std::env::set_var("KAFKA_BROKERS", "localhost:9092");
            }

            if enable_rabbitmq {
                std::env::set_var("RABBITMQ_ADDR", "amqp://guest:guest@127.0.0.1:5672/%2f");
            }

            let mut config = ScouterServerConfig::default();
            config.polling_settings.num_workers = 1;

            let app = create_app(config.clone()).await?;

            let db_client = PostgresClient::new(None, None)
                .await
                .with_context(|| "Failed to create Postgres client")?;

            cleanup(&db_client.pool).await?;

            Ok(Self { app })
        }
        pub async fn send_oneshot(&self, request: Request<Body>) -> Response<Body> {
            self.app.clone().oneshot(request).await.unwrap()
        }

        pub fn get_data(&self) -> (Array<f64, ndarray::Dim<[usize; 2]>>, Vec<String>) {
            let array = Array::random((1030, 3), Uniform::new(0., 10.));

            let features = vec![
                "feature_1".to_string(),
                "feature_2".to_string(),
                "feature_3".to_string(),
            ];

            (array, features)
        }

        pub fn get_spc_drift_records(&self) -> ServerRecords {
            let mut records: Vec<ServerRecord> = Vec::new();
            let record_type = RecordType::Spc;
            for _ in 0..10 {
                for j in 0..10 {
                    let record = SpcServerRecord {
                        created_at: chrono::Utc::now().naive_utc(),
                        name: "test".to_string(),
                        repository: "test".to_string(),
                        version: "test".to_string(),
                        feature: format!("test{}", j),
                        value: j as f64,
                        record_type: RecordType::Spc,
                    };

                    records.push(ServerRecord::Spc(record));
                }
            }

            ServerRecords::new(records, record_type)
        }

        pub fn get_psi_drift_records(&self) -> ServerRecords {
            let mut records: Vec<ServerRecord> = Vec::new();

            for feature in 1..3 {
                for decile in 0..10 {
                    for _ in 0..100 {
                        let record = PsiServerRecord {
                            created_at: chrono::Utc::now().naive_utc(),
                            name: "test".to_string(),
                            repository: "test".to_string(),
                            version: "1.0.0".to_string(),
                            feature: format!("feature_{}", feature),
                            bin_id: decile,
                            bin_count: rand::thread_rng().gen_range(0..10),
                            record_type: RecordType::Psi,
                        };

                        records.push(ServerRecord::Psi(record));
                    }
                }
            }
            ServerRecords::new(records, RecordType::Psi)
        }

        pub fn get_custom_drift_records(&self) -> ServerRecords {
            let mut records: Vec<ServerRecord> = Vec::new();
            for i in 0..2 {
                for _ in 0..25 {
                    let record = CustomMetricServerRecord {
                        created_at: chrono::Utc::now().naive_utc(),
                        name: "test".to_string(),
                        repository: "test".to_string(),
                        version: "test".to_string(),
                        metric: format!("metric{}", i),
                        value: rand::thread_rng().gen_range(0..10) as f64,
                        record_type: RecordType::Custom,
                    };

                    records.push(ServerRecord::Custom(record));
                }
            }

            ServerRecords::new(records, RecordType::Custom)
        }
    }

    #[tokio::test]
    async fn test_health_check() {
        let helper = TestHelper::new(false, false).await.unwrap();

        let request = Request::builder()
            .uri("/scouter/healthcheck")
            .body(Body::empty())
            .unwrap();

        let response = helper.send_oneshot(request).await;

        //assert response
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();

        let v: Alive = serde_json::from_slice(&body).unwrap();

        assert_eq!(v.status, "Alive");
    }

    #[tokio::test]
    async fn test_create_spc_profile() {
        let helper = TestHelper::new(false, false).await.unwrap();

        let (array, features) = helper.get_data();
        let alert_config = SpcAlertConfig::default();
        let config = SpcDriftConfig::new(
            Some("name".to_string()),
            Some("repo".to_string()),
            None,
            None,
            None,
            None,
            None,
            Some(alert_config),
            None,
        );

        let monitor = SpcMonitor::new();

        let mut profile = monitor
            .create_2d_drift_profile(&features, &array.view(), &config.unwrap())
            .unwrap();

        let request = ProfileRequest {
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

        // update profile
        profile.config.sample_size = 100;

        assert_eq!(profile.config.sample_size, 100);

        let request = ProfileRequest {
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

        // get profile
        let params = GetProfileRequest {
            name: profile.config.name.clone(),
            repository: profile.config.repository.clone(),
            version: profile.config.version.clone(),
            drift_type: DriftType::Spc,
        };

        let query_string = serde_qs::to_string(&params).unwrap();

        let request = Request::builder()
            .uri(format!("/scouter/profile?{}", query_string))
            .method("GET")
            .body(Body::empty())
            .unwrap();

        let response = helper.send_oneshot(request).await;

        //assert response
        assert_eq!(response.status(), StatusCode::OK);

        // update profile status
        let request = ProfileStatusRequest {
            name: profile.config.name.clone(),
            repository: profile.config.repository.clone(),
            version: profile.config.version.clone(),
            active: true,
        };

        let body = serde_json::to_string(&request).unwrap();

        let request = Request::builder()
            .uri("/scouter/profile/status")
            .method("PUT")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = helper.send_oneshot(request).await;

        //assert response
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_spc_server_records() {
        let helper = TestHelper::new(false, false).await.unwrap();
        let records = helper.get_spc_drift_records();
        let body = serde_json::to_string(&records).unwrap();

        let request = Request::builder()
            .uri("/scouter/drift")
            .method("POST")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = helper.send_oneshot(request).await;

        //assert response
        assert_eq!(response.status(), StatusCode::OK);

        // get drift records
        let params = DriftRequest {
            name: "test".to_string(),
            repository: "test".to_string(),
            version: "test".to_string(),
            time_window: TimeInterval::FiveMinutes,
            max_data_points: 100,
            drift_type: DriftType::Spc,
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

        let body = response.into_body().collect().await.unwrap().to_bytes();

        let results: Vec<SpcFeatureResult> = serde_json::from_slice(&body).unwrap();

        assert_eq!(results.len(), 10);
    }

    #[tokio::test]
    async fn test_psi_server_records() {
        let helper = TestHelper::new(false, false).await.unwrap();

        let (array, features) = helper.get_data();
        let alert_config = PsiAlertConfig::new(
            None,
            None,
            Some(vec![
                "feature_1".to_string(),
                "feature_2".to_string(),
                "feature_3".to_string(),
            ]),
            None,
            None,
        );
        let config = PsiDriftConfig::new(
            Some("test".to_string()),
            Some("test".to_string()),
            Some("1.0.0".to_string()),
            None,
            None,
            Some(alert_config),
            None,
        );

        let monitor = PsiMonitor::new();

        let profile = monitor
            .create_2d_drift_profile(&features, &array.view(), &config.unwrap())
            .unwrap();

        let request = ProfileRequest {
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

        let records = helper.get_psi_drift_records();
        let body = serde_json::to_string(&records).unwrap();

        let request = Request::builder()
            .uri("/scouter/drift")
            .method("POST")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = helper.send_oneshot(request).await;

        //assert response
        assert_eq!(response.status(), StatusCode::OK);

        // get drift records
        let params = DriftRequest {
            name: "test".to_string(),
            repository: "test".to_string(),
            version: "1.0.0".to_string(),
            time_window: TimeInterval::FiveMinutes,
            max_data_points: 100,
            drift_type: DriftType::Psi,
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

        // collect body into serde Value

        let val = response.into_body().collect().await.unwrap().to_bytes();

        let results: BinnedPsiFeatureMetrics = serde_json::from_slice(&val).unwrap();

        assert!(!results.features.is_empty());

        // get psi viz data
        let request = Request::builder()
            .uri(format!("/scouter/drift/psi/viz?{}", query_string))
            .method("GET")
            .body(Body::empty())
            .unwrap();

        let response = helper.send_oneshot(request).await;

        //assert response
        assert_eq!(response.status(), StatusCode::OK);

        // collect body into serde Value

        let val = response.into_body().collect().await.unwrap().to_bytes();

        let _results: PsiDriftViz = serde_json::from_slice(&val).unwrap();
    }

    #[tokio::test]
    async fn test_custom_server_records() {
        let helper = TestHelper::new(false, false).await.unwrap();

        let alert_config = CustomMetricAlertConfig::new(None, None, None);
        let config = CustomMetricDriftConfig::new(
            Some("test".to_string()),
            Some("test".to_string()),
            Some("1.0.0".to_string()),
            Some(alert_config),
            None,
        )
        .unwrap();

        let alert_threshold = AlertThreshold::Above;
        let metric1 =
            CustomMetric::new("metric1".to_string(), 1.0, alert_threshold.clone(), None).unwrap();
        let metric2 = CustomMetric::new("metric2".to_string(), 1.0, alert_threshold, None).unwrap();
        let profile = CustomDriftProfile::new(config, vec![metric1, metric2], None).unwrap();

        let request = ProfileRequest {
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

        let records = helper.get_custom_drift_records();
        let body = serde_json::to_string(&records).unwrap();

        let request = Request::builder()
            .uri("/scouter/drift")
            .method("POST")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = helper.send_oneshot(request).await;

        //assert response
        assert_eq!(response.status(), StatusCode::OK);

        // get drift records
        let params = DriftRequest {
            name: "test".to_string(),
            repository: "test".to_string(),
            version: "1.0.0".to_string(),
            time_window: TimeInterval::FiveMinutes,
            max_data_points: 100,
            drift_type: DriftType::Custom,
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

        // collect body into serde Value

        let val = response.into_body().collect().await.unwrap().to_bytes();

        let results: BinnedCustomMetrics = serde_json::from_slice(&val).unwrap();

        assert!(!results.metrics.is_empty());
    }
}
