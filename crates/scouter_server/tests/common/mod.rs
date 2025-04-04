use anyhow::Context;
use axum::response::Response;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use chrono::Utc;
use http_body_util::BodyExt;
use rand::Rng;
use scouter_server::create_app;
use scouter_types::{CustomMetricServerRecord, PsiServerRecord};
use scouter_types::{ServerRecord, ServerRecords, SpcServerRecord};
// for `collect`

use ndarray::Array;
use ndarray_rand::rand_distr::Uniform;
use ndarray_rand::RandomExt;
use scouter_events::producer::http::types::JwtToken;
use scouter_sql::PostgresClient;
use sqlx::{PgPool, Pool, Postgres};
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
        FROM scouter.drift_alert;

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
    pool: PgPool,
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

        Ok(Self {
            app,
            token,
            pool: db_client.pool.clone(),
        })
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

        for _ in 0..10 {
            for j in 0..10 {
                let record = SpcServerRecord {
                    created_at: Utc::now(),
                    name: "test".to_string(),
                    space: "test".to_string(),
                    version: "test".to_string(),
                    feature: format!("test{}", j),
                    value: j as f64,
                };

                records.push(ServerRecord::Spc(record));
            }
        }

        ServerRecords::new(records)
    }

    pub fn get_psi_drift_records(&self) -> ServerRecords {
        let mut records: Vec<ServerRecord> = Vec::new();

        for feature in 1..3 {
            for decile in 0..10 {
                for _ in 0..100 {
                    // add one minute to each record
                    let record = PsiServerRecord {
                        created_at: Utc::now(),
                        name: "test".to_string(),
                        space: "test".to_string(),
                        version: "1.0.0".to_string(),
                        feature: format!("feature_{}", feature),
                        bin_id: decile,
                        bin_count: rand::rng().random_range(0..10),
                    };

                    records.push(ServerRecord::Psi(record));
                }
            }
        }
        ServerRecords::new(records)
    }

    pub fn get_custom_drift_records(&self) -> ServerRecords {
        let mut records: Vec<ServerRecord> = Vec::new();
        for i in 0..2 {
            for _ in 0..25 {
                let record = CustomMetricServerRecord {
                    created_at: Utc::now(),
                    name: "test".to_string(),
                    space: "test".to_string(),
                    version: "1.0.0".to_string(),
                    metric: format!("metric{}", i),
                    value: rand::rng().random_range(0..10) as f64,
                };

                records.push(ServerRecord::Custom(record));
            }
        }

        ServerRecords::new(records)
    }

    pub async fn insert_alerts(&self) -> Result<(), anyhow::Error> {
        // Run the SQL script to populate the database
        let script = std::fs::read_to_string("tests/fixtures/populate_alerts.sql").unwrap();

        sqlx::query(&script).execute(&self.pool).await.unwrap();

        Ok(())
    }
}
