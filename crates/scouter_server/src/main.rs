pub mod api;

use crate::api::middleware::metrics::metrics_app;
use crate::api::shutdown::{shutdown_metric_signal, shutdown_signal};
use crate::api::state::AppState;
use anyhow::Context;
use api::router::create_router;
use api::setup::setup_components;
use axum::Router;
use scouter_auth::auth::AuthManager;
use std::sync::Arc;
use tracing::info;

/// Start the metrics server for prometheus
async fn start_metrics_server() -> Result<(), anyhow::Error> {
    let app = metrics_app().with_context(|| "Failed to setup metrics app")?;

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001")
        .await
        .with_context(|| "Failed to bind to port 3001 for metrics server")?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_metric_signal())
        .await
        .with_context(|| "Failed to start metrics server")?;

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
async fn create_app() -> Result<(Router, Arc<AppState>), anyhow::Error> {
    // setup logging, soft fail if it fails

    let (config, db_client, shutdown_tx) = setup_components().await?;

    let app_state = Arc::new(AppState {
        db: db_client,
        shutdown_tx,
        auth_manager: AuthManager::new(
            &config.auth_settings.jwt_secret,
            &config.auth_settings.refresh_secret,
        ),
        config,
    });

    let router = create_router(app_state.clone())
        .await
        .with_context(|| "Failed to create router")?;

    Ok((router, app_state))
}

/// Start the main server
async fn start_main_server() -> Result<(), anyhow::Error> {
    let (router, app_state) = create_app().await?;

    let port = std::env::var("OPSML_SERVER_PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(addr.clone())
        .await
        .with_context(|| "Failed to bind to port 8000")?;

    info!(
        "🚀 Scouter Server started successfully on {:?}",
        addr.clone().to_string()
    );
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal(app_state.clone()))
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
    use crate::api::routes::user::schema::{
        CreateUserRequest, UpdateUserRequest, UserListResponse, UserResponse,
    };

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
    use scouter_types::custom::{
        AlertThreshold, BinnedCustomMetrics, CustomDriftProfile, CustomMetric,
        CustomMetricAlertConfig, CustomMetricDriftConfig,
    };
    use scouter_types::psi::BinnedPsiFeatureMetrics;
    use scouter_types::psi::{PsiAlertConfig, PsiDriftConfig};
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
    use scouter_events::producer::http::types::JwtToken;
    use scouter_sql::PostgresClient;
    use scouter_types::spc::{SpcAlertConfig, SpcDriftConfig, SpcDriftFeatures};
    use sqlx::{Pool, Postgres};
    use std::env;
    use tower::util::ServiceExt;

    pub async fn cleanup(pool: &Pool<Postgres>) -> Result<(), anyhow::Error> {
        sqlx::raw_sql(
            r#"
            DELETE
            FROM scouter.drift;

            DELETE
            FROM scouter.observability_metrics;

            DELETE
            FROM scouter.custom_metrics;

            DELETE
            FROM scouter.drift_alerts;

            DELETE
            FROM scouter.drift_profile;

            DELETE
            FROM scouter.scouter_users;

            DELETE
            FROM scouter.observed_bin_count;
            "#,
        )
        .fetch_all(pool)
        .await
        .unwrap();

        Ok(())
    }

    pub struct TestHelper {
        app: Router,
        token: JwtToken,
    }

    impl TestHelper {
        pub async fn new(enable_kafka: bool, enable_rabbitmq: bool) -> Result<Self, anyhow::Error> {
            env::set_var("RUST_LOG", "debug");
            env::set_var("LOG_LEVEL", "debug");
            env::set_var("LOG_JSON", "false");
            env::set_var("POLLING_WORKER_COUNT", "1");

            if enable_kafka {
                std::env::set_var("KAFKA_BROKERS", "localhost:9092");
            }

            if enable_rabbitmq {
                std::env::set_var("RABBITMQ_ADDR", "amqp://guest:guest@127.0.0.1:5672/%2f");
            }

            let db_client = PostgresClient::new(None, None)
                .await
                .with_context(|| "Failed to create Postgres client")?;

            cleanup(&db_client.pool).await?;

            let (app, _app_state) = create_app().await?;
            let token = TestHelper::login(&app).await;

            Ok(Self { app, token })
        }

        pub async fn login(app: &Router) -> JwtToken {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/scouter/auth/login")
                        .header("Username", "admin")
                        .header("Password", "admin")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::OK);

            let body = response.into_body().collect().await.unwrap().to_bytes();
            let token: JwtToken = serde_json::from_slice(&body).unwrap();

            token
        }

        pub fn with_auth_header(&self, mut request: Request<Body>) -> Request<Body> {
            request.headers_mut().insert(
                header::AUTHORIZATION,
                format!("Bearer {}", self.token.token).parse().unwrap(),
            );

            request
        }

        pub async fn send_oneshot(&self, request: Request<Body>) -> Response<Body> {
            self.app
                .clone()
                .oneshot(self.with_auth_header(request))
                .await
                .unwrap()
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
                        // add one minute to each record
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
                        version: "1.0.0".to_string(),
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
            Some(alert_config),
            None,
        );

        let monitor = SpcMonitor::new();

        let mut profile = monitor
            .create_2d_drift_profile(&features, &array.view(), &config.unwrap())
            .unwrap();

        let request = ProfileRequest {
            repository: profile.config.repository.clone(),
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
            repository: profile.config.repository.clone(),
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
            drift_type: None,
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
            time_interval: TimeInterval::FiveMinutes,
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

        let results: SpcDriftFeatures = serde_json::from_slice(&body).unwrap();

        assert_eq!(results.features.len(), 10);
    }

    #[tokio::test]
    async fn test_psi_server_records() {
        let helper = TestHelper::new(false, false).await.unwrap();

        let (array, features) = helper.get_data();
        let alert_config = PsiAlertConfig {
            features_to_monitor: vec![
                "feature_1".to_string(),
                "feature_2".to_string(),
                "feature_3".to_string(),
            ],
            ..Default::default()
        };

        let config = PsiDriftConfig::new("test", "test", "1.0.0", None, alert_config, None);

        let monitor = PsiMonitor::new();

        let profile = monitor
            .create_2d_drift_profile(&features, &array.view(), &config.unwrap())
            .unwrap();

        let request = ProfileRequest {
            repository: profile.config.repository.clone(),
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
            time_interval: TimeInterval::FiveMinutes,
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
    }

    #[tokio::test]
    async fn test_custom_server_records() {
        let helper = TestHelper::new(false, false).await.unwrap();

        let alert_config = CustomMetricAlertConfig::default();
        let config =
            CustomMetricDriftConfig::new("test", "test", "1.0.0", true, 25, alert_config, None)
                .unwrap();

        let alert_threshold = AlertThreshold::Above;
        let metric1 = CustomMetric::new("metric1", 1.0, alert_threshold.clone(), None).unwrap();
        let metric2 = CustomMetric::new("metric2", 1.0, alert_threshold, None).unwrap();
        let profile = CustomDriftProfile::new(config, vec![metric1, metric2], None).unwrap();

        let request = ProfileRequest {
            repository: profile.config.repository.clone(),
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
            time_interval: TimeInterval::FiveMinutes,
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

    #[tokio::test]
    async fn test_server_user_crud() {
        let helper = TestHelper::new(false, false).await.unwrap();

        // 1. Create a new user
        let create_req = CreateUserRequest {
            username: "test_user".to_string(),
            password: "test_password".to_string(),
            permissions: Some(vec!["read".to_string(), "write".to_string()]),
            group_permissions: Some(vec!["user".to_string()]),
            role: Some("user".to_string()),
            active: Some(true),
        };

        let body = serde_json::to_string(&create_req).unwrap();

        let request = Request::builder()
            .uri("/scouter/users")
            .method("POST")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = helper.send_oneshot(request).await;
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let user_response: UserResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(user_response.username, "test_user");
        assert_eq!(
            user_response.permissions,
            vec!["read".to_string(), "write".to_string()]
        );
        assert_eq!(user_response.group_permissions, vec!["user".to_string()]);
        assert!(user_response.active);

        // 2. Get the user
        let request = Request::builder()
            .uri("/scouter/users/test_user")
            .method("GET")
            .body(Body::empty())
            .unwrap();

        let response = helper.send_oneshot(request).await;
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let user_response: UserResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(user_response.username, "test_user");

        // 3. Update the user
        let update_req = UpdateUserRequest {
            password: Some("new_password".to_string()),
            permissions: Some(vec![
                "read".to_string(),
                "write".to_string(),
                "execute".to_string(),
            ]),
            group_permissions: Some(vec!["user".to_string(), "developer".to_string()]),
            active: Some(true),
        };

        let body = serde_json::to_string(&update_req).unwrap();

        let request = Request::builder()
            .uri("/scouter/users/test_user")
            .method("PUT")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = helper.send_oneshot(request).await;
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let user_response: UserResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            user_response.permissions,
            vec![
                "read".to_string(),
                "write".to_string(),
                "execute".to_string()
            ]
        );
        assert_eq!(
            user_response.group_permissions,
            vec!["user".to_string(), "developer".to_string()]
        );

        // 4. List all users
        let request = Request::builder()
            .uri("/scouter/users")
            .method("GET")
            .body(Body::empty())
            .unwrap();

        let response = helper.send_oneshot(request).await;
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let list_response: UserListResponse = serde_json::from_slice(&body).unwrap();

        // Find our test user in the list
        let test_user = list_response
            .users
            .iter()
            .find(|u| u.username == "test_user");
        assert!(test_user.is_some());

        // 5. Delete the user
        let request = Request::builder()
            .uri("/scouter/users/test_user")
            .method("DELETE")
            .body(Body::empty())
            .unwrap();

        let response = helper.send_oneshot(request).await;
        assert_eq!(response.status(), StatusCode::OK);

        // Verify the user is deleted by trying to get it
        let request = Request::builder()
            .uri("/scouter/users/test_user")
            .method("GET")
            .body(Body::empty())
            .unwrap();

        let response = helper.send_oneshot(request).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
