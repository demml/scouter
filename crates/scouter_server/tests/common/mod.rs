use anyhow::Context;
use axum::response::Response;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use chrono::Utc;
use http_body_util::BodyExt;
use ndarray::Array;
use ndarray_rand::rand_distr::Uniform;
use ndarray_rand::RandomExt;
use potato_head::create_score_prompt;
use rand::Rng;
use scouter_server::create_app;
use scouter_settings::ObjectStorageSettings;
use scouter_settings::{DatabaseSettings, ScouterServerConfig};
use scouter_sql::PostgresClient;
use scouter_types::JwtToken;
use scouter_types::{
    llm::{LLMAlertConfig, LLMDriftConfig, LLMDriftMetric, LLMDriftProfile},
    AlertThreshold, CustomMetricServerRecord, LLMMetricRecord, MessageRecord, PsiServerRecord,
};
use scouter_types::{
    BoxedLLMDriftServerRecord, LLMDriftServerRecord, ServerRecord, ServerRecords, SpcServerRecord,
    Status,
};
use serde_json::Value;
use sqlx::{PgPool, Pool, Postgres};
use std::env;
use std::sync::Arc;
use tower::util::ServiceExt;

pub const SPACE: &str = "space";
pub const NAME: &str = "name";
pub const VERSION: &str = "1.0.0";

pub async fn cleanup(pool: &Pool<Postgres>) -> Result<(), anyhow::Error> {
    sqlx::raw_sql(
        r#"
        DELETE
        FROM scouter.spc_drift;

        DELETE
        FROM scouter.observability_metric;

        DELETE
        FROM scouter.custom_drift;

        DELETE
        FROM scouter.drift_alert;

        DELETE
        FROM scouter.drift_profile;

        DELETE
        FROM scouter.user;

        DELETE
        FROM scouter.psi_drift;

        DELETE
        FROM scouter.llm_drift_record;

        DELETE
        FROM scouter.llm_drift;

        DELETE
        FROM scouter.spans;

        DELETE
        FROM scouter.trace_baggage;

        DELETE
        FROM scouter.traces;

        DELETE
        FROM scouter.tags;
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
    pub pool: PgPool,
    pub config: Arc<ScouterServerConfig>,
}

impl TestHelper {
    pub fn cleanup_storage() {
        let storage_settings = ObjectStorageSettings::default();
        let current_dir = std::env::current_dir().unwrap();
        let storage_path = current_dir.join(storage_settings.storage_root());
        if storage_path.exists() {
            std::fs::remove_dir_all(storage_path).unwrap();
        }
    }
    pub async fn new(enable_kafka: bool, enable_rabbitmq: bool) -> Result<Self, anyhow::Error> {
        TestHelper::cleanup_storage();

        env::set_var("RUST_LOG", "info");
        env::set_var("LOG_LEVEL", "inf");
        env::set_var("LOG_JSON", "false");
        env::set_var("POLLING_WORKER_COUNT", "1");
        env::set_var("DATA_RETENTION_PERIOD", "5");
        std::env::set_var("OPENAI_API_KEY", "test_key");

        if enable_kafka {
            std::env::set_var("KAFKA_BROKERS", "localhost:9092");
        }

        if enable_rabbitmq {
            std::env::set_var("RABBITMQ_ADDR", "amqp://guest:guest@127.0.0.1:5672/%2f");
        }

        let db_pool = PostgresClient::create_db_pool(&DatabaseSettings::default())
            .await
            .context("Failed to create Postgres client")?;

        cleanup(&db_pool).await?;

        let (app, app_state) = create_app().await?;
        let token = TestHelper::login(&app).await;

        Ok(Self {
            app,
            token,
            pool: db_pool,
            config: app_state.config.clone(),
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

    pub fn get_spc_drift_records(&self, time_offset: Option<i64>) -> MessageRecord {
        let mut records: Vec<ServerRecord> = Vec::new();
        let offset = time_offset.unwrap_or(0);

        for _ in 0..10 {
            for j in 0..10 {
                let record = SpcServerRecord {
                    created_at: Utc::now() - chrono::Duration::days(offset),
                    space: SPACE.to_string(),
                    name: NAME.to_string(),
                    version: VERSION.to_string(),
                    feature: format!("feature_{j}"),
                    value: j as f64,
                };

                records.push(ServerRecord::Spc(record));
            }
        }

        MessageRecord::ServerRecords(ServerRecords::new(records))
    }

    pub fn get_psi_drift_records(&self, time_offset: Option<i64>) -> MessageRecord {
        let mut records: Vec<ServerRecord> = Vec::new();
        let offset = time_offset.unwrap_or(0);

        for feature in 1..3 {
            for decile in 0..10 {
                for _ in 0..100 {
                    // add one minute to each record
                    let record = PsiServerRecord {
                        created_at: Utc::now() - chrono::Duration::days(offset),
                        space: SPACE.to_string(),
                        name: NAME.to_string(),
                        version: VERSION.to_string(),
                        feature: format!("feature_{feature}"),
                        bin_id: decile,
                        bin_count: rand::rng().random_range(0..10),
                    };

                    records.push(ServerRecord::Psi(record));
                }
            }
        }
        MessageRecord::ServerRecords(ServerRecords::new(records))
    }

    pub fn get_custom_drift_records(&self, time_offset: Option<i64>) -> MessageRecord {
        let mut records: Vec<ServerRecord> = Vec::new();
        let offset = time_offset.unwrap_or(0);
        for i in 0..2 {
            for _ in 0..50 {
                let record = CustomMetricServerRecord {
                    created_at: Utc::now() - chrono::Duration::days(offset),
                    space: SPACE.to_string(),
                    name: NAME.to_string(),
                    version: VERSION.to_string(),
                    metric: format!("metric_{i}"),
                    value: rand::rng().random_range(0..10) as f64,
                };

                records.push(ServerRecord::Custom(record));
            }
        }

        MessageRecord::ServerRecords(ServerRecords::new(records))
    }

    pub fn get_llm_drift_records(&self, time_offset: Option<i64>) -> MessageRecord {
        let mut records: Vec<ServerRecord> = Vec::new();
        let offset = time_offset.unwrap_or(0);
        let prompt = create_score_prompt(None);

        for i in 0..3 {
            for _ in 0..5 {
                let context = serde_json::json!({
                    "input": format!("input{i}"),
                    "response": format!("output{i}"),
                });
                let record = LLMDriftServerRecord {
                    created_at: Utc::now() - chrono::Duration::days(offset),
                    space: SPACE.to_string(),
                    name: NAME.to_string(),
                    version: VERSION.to_string(),
                    prompt: Some(prompt.model_dump_value()),
                    context,
                    status: Status::Pending,
                    id: 0,
                    uid: "test-uid".to_string(),
                    updated_at: None,
                    processing_started_at: None,
                    processing_ended_at: None,
                    score: Value::Null,
                    processing_duration: None,
                };

                let boxed_record = BoxedLLMDriftServerRecord::new(record);
                records.push(ServerRecord::LLMDrift(boxed_record));
            }
        }

        MessageRecord::ServerRecords(ServerRecords::new(records))
    }

    pub fn get_llm_drift_metrics(&self, time_offset: Option<i64>) -> MessageRecord {
        let mut records: Vec<ServerRecord> = Vec::new();
        let offset = time_offset.unwrap_or(0);

        for i in 0..2 {
            for j in 0..25 {
                let record = LLMMetricRecord {
                    record_uid: format!("record_uid_{i}_{j}"),
                    created_at: Utc::now() + chrono::Duration::microseconds(j as i64)
                        - chrono::Duration::days(offset),
                    space: SPACE.to_string(),
                    name: NAME.to_string(),
                    version: VERSION.to_string(),
                    metric: format!("metric{i}"),
                    value: rand::rng().random_range(0..3) as f64,
                };
                records.push(ServerRecord::LLMMetric(record));
            }
        }

        MessageRecord::ServerRecords(ServerRecords::new(records))
    }

    pub async fn create_llm_drift_profile() -> LLMDriftProfile {
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
            Some(0.5),
            Some(prompt.clone()),
        )
        .unwrap();
        let llm_metrics = vec![metric1, metric2];
        LLMDriftProfile::from_metrics(config, llm_metrics)
            .await
            .unwrap()
    }

    pub async fn insert_alerts(&self) -> Result<(), anyhow::Error> {
        // Run the SQL script to populate the database
        let script = std::fs::read_to_string("tests/fixtures/populate_alerts.sql").unwrap();

        sqlx::query(&script).execute(&self.pool).await.unwrap();

        Ok(())
    }

    pub async fn generate_trace_data(&self) -> Result<(), anyhow::Error> {
        //print current dir
        println!("Current dir: {:?}", std::env::current_dir().unwrap());
        let script =
            std::fs::read_to_string("../scouter_sql/src/tests/script/populate_trace.sql").unwrap();
        sqlx::query(&script).execute(&self.pool).await.unwrap();

        Ok(())
    }

    pub async fn get_db_pool(&self) -> Pool<Postgres> {
        PostgresClient::create_db_pool(&DatabaseSettings::default())
            .await
            .context("Failed to create Postgres client")
            .unwrap()
    }
}
