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
use potato_head::{create_uuid7, mock::create_score_prompt};
use rand::Rng;
use scouter_drift::spc::SpcMonitor;
use scouter_server::api::grpc::start_grpc_server;
use scouter_server::api::state::AppState;
use scouter_server::{create_app_state, create_http_router};
use scouter_settings::grpc::GrpcConfig;
use scouter_settings::ObjectStorageSettings;
use scouter_settings::{DatabaseSettings, ScouterServerConfig};
use scouter_sql::sql::traits::AlertSqlLogic;
use scouter_sql::sql::traits::EntitySqlLogic;
use scouter_sql::PostgresClient;
use scouter_tonic::GrpcClient;
use scouter_types::spc::SpcDriftConfig;
use scouter_types::spc::{SpcAlertConfig, SpcDriftProfile};
use scouter_types::JwtToken;
use scouter_types::RegisteredProfileResponse;
use scouter_types::{
    genai::{
        ComparisonOperator, EvaluationTasks, GenAIAlertConfig, GenAIDriftConfig, GenAIEvalProfile,
        LLMJudgeTask,
    },
    CustomMetricRecord, GenAIEvalTaskResult, GenAIEvalWorkflowResult, MessageRecord, PsiRecord,
};
use scouter_types::{
    BoxedGenAIEvalRecord, GenAIEvalRecord, ServerRecord, ServerRecords, SpcRecord, Status,
};
use scouter_types::{DriftType, RecordType};
use serde_json::Value;
use sqlx::{PgPool, Pool, Postgres};
use std::collections::BTreeMap;
use std::env;
use std::sync::Arc;
use tokio::runtime;

use tower::util::ServiceExt;
use tracing::error;

pub const SPACE: &str = "space";
pub const NAME: &str = "name";
pub const VERSION: &str = "1.0.0";
pub const ENTITY_ID_PLACEHOLDER: i32 = 0;

pub async fn cleanup_tables(pool: &Pool<Postgres>) -> Result<(), anyhow::Error> {
    sqlx::raw_sql(
        r#"
        DELETE
        FROM scouter.spc_drift;

        DELETE
        FROM scouter.drift_entities;

        DELETE
        FROM scouter.service_entities;

        DELETE
        FROM scouter.observability_metric;

        DELETE
        FROM scouter.custom_drift;

        DELETE
        FROM scouter.drift_alert;

        DELETE
        FROM scouter.drift_profile;

        DELETE
        FROM scouter.psi_drift;

        DELETE
        FROM scouter.genai_eval_record;

        DELETE
        FROM scouter.genai_eval_workflow;

        DELETE
        FROM scouter.genai_eval_task;

        DELETE
        FROM scouter.spans;

        DELETE
        FROM scouter.trace_baggage;

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
    router: Router,
    token: JwtToken,
    grpc_handle: tokio::task::JoinHandle<()>,
    grpc_shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    pub pool: PgPool,
    pub config: Arc<ScouterServerConfig>,
}

impl TestHelper {
    pub async fn start_grpc_test_server(
        state: Arc<AppState>,
        shutdown_rx: tokio::sync::oneshot::Receiver<()>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            tokio::select! {
                result = start_grpc_server(state) => {
                    if let Err(e) = result {
                        error!("gRPC test server error: {}", e);
                    }
                }
                _ = shutdown_rx => {
                    tracing::info!("gRPC test server received shutdown signal");
                }
            }
        })
    }

    /// Gracefully shutdown the gRPC server
    pub async fn shutdown_grpc_server(&mut self) {
        if let Some(tx) = self.grpc_shutdown_tx.take() {
            let _ = tx.send(()); // Signal shutdown

            // Wait for graceful shutdown with timeout
            let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(5));
            tokio::select! {
                _ = &mut self.grpc_handle => {
                    tracing::info!("gRPC server shutdown gracefully");
                }
                _ = timeout => {
                    self.grpc_handle.abort();
                    tracing::warn!("gRPC server shutdown timed out, aborted");
                }
            }
        }
    }

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

        unsafe {
            env::set_var("RUST_LOG", "info");
            env::set_var("LOG_LEVEL", "inf");
            env::set_var("LOG_JSON", "false");
            env::set_var("POLLING_WORKER_COUNT", "1");
            env::set_var("MAX_POOL_SIZE", "100");
            env::set_var("DATA_RETENTION_PERIOD", "5");
            std::env::set_var("OPENAI_API_KEY", "test_key");
        }

        if enable_kafka {
            unsafe {
                std::env::set_var("KAFKA_BROKERS", "localhost:9092");
            }
        }

        if enable_rabbitmq {
            unsafe {
                std::env::set_var("RABBITMQ_ADDR", "amqp://guest:guest@127.0.0.1:5672/%2f");
            }
        }

        let app_state = create_app_state().await?;
        let router = create_http_router(app_state.clone()).await?;
        let (grpc_shutdown_tx, grpc_shutdown_rx) = tokio::sync::oneshot::channel();
        let grpc_handle =
            TestHelper::start_grpc_test_server(app_state.clone(), grpc_shutdown_rx).await;

        let token = TestHelper::login(&router).await;

        let db_pool = app_state.db_pool.clone();
        //cleanup(&db_pool).await?;

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        Ok(Self {
            router,
            grpc_handle,
            grpc_shutdown_tx: Some(grpc_shutdown_tx),
            token,
            pool: db_pool,
            config: app_state.config.clone(),
        })
    }

    pub async fn create_grpc_client(&self) -> GrpcClient {
        GrpcClient::new(GrpcConfig::default()).await.unwrap()
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
        self.router
            .clone()
            .oneshot(self.with_auth_header(request))
            .await
            .unwrap()
    }

    pub fn get_data(&self) -> (Array<f64, ndarray::Dim<[usize; 2]>>, Vec<String>) {
        let array = Array::random((1030, 3), Uniform::new(0., 10.).unwrap());

        let features = vec![
            "feature_1".to_string(),
            "feature_2".to_string(),
            "feature_3".to_string(),
        ];

        (array, features)
    }

    pub fn get_spc_drift_records(&self, time_offset: Option<i64>, uid: &str) -> MessageRecord {
        let mut records: Vec<ServerRecord> = Vec::new();
        let offset = time_offset.unwrap_or(0);

        for _ in 0..10 {
            for j in 0..10 {
                let record = SpcRecord {
                    created_at: Utc::now() - chrono::Duration::days(offset),
                    uid: uid.to_string(),
                    feature: format!("feature_{j}"),
                    value: j as f64,
                    entity_id: ENTITY_ID_PLACEHOLDER,
                };

                records.push(ServerRecord::Spc(record));
            }
        }

        MessageRecord::ServerRecords(ServerRecords::new(records))
    }

    pub fn get_psi_drift_records(&self, time_offset: Option<i64>, uid: &str) -> MessageRecord {
        let mut records: Vec<ServerRecord> = Vec::new();
        let offset = time_offset.unwrap_or(0);

        for feature in 1..3 {
            for decile in 0..10 {
                for _ in 0..100 {
                    // add one minute to each record
                    let record = PsiRecord {
                        created_at: Utc::now() - chrono::Duration::days(offset),
                        uid: uid.to_string(),
                        feature: format!("feature_{feature}"),
                        bin_id: decile,
                        bin_count: rand::rng().random_range(0..10),
                        entity_id: ENTITY_ID_PLACEHOLDER,
                    };

                    records.push(ServerRecord::Psi(record));
                }
            }
        }
        MessageRecord::ServerRecords(ServerRecords::new(records))
    }

    pub fn get_custom_drift_records(&self, time_offset: Option<i64>, uid: &str) -> MessageRecord {
        let mut records: Vec<ServerRecord> = Vec::new();
        let offset = time_offset.unwrap_or(0);
        for i in 0..2 {
            for _ in 0..50 {
                let record = CustomMetricRecord {
                    created_at: Utc::now() - chrono::Duration::days(offset),
                    uid: uid.to_string(),
                    metric: format!("metric_{i}"),
                    value: rand::rng().random_range(0..10) as f64,
                    entity_id: ENTITY_ID_PLACEHOLDER,
                };

                records.push(ServerRecord::Custom(record));
            }
        }

        MessageRecord::ServerRecords(ServerRecords::new(records))
    }

    pub fn get_genai_event_records(time_offset: Option<i64>, uid: &str) -> MessageRecord {
        let mut records: Vec<ServerRecord> = Vec::new();
        let offset = time_offset.unwrap_or(0);

        for i in 0..3 {
            for _ in 0..5 {
                let context = serde_json::json!({
                    "input": format!("input{i}"),
                    "response": format!("output{i}"),
                });
                let record = GenAIEvalRecord {
                    created_at: Utc::now() - chrono::Duration::days(offset),
                    entity_uid: uid.to_string(),
                    context,
                    status: Status::Pending,
                    id: 0,
                    uid: create_uuid7(),
                    entity_id: ENTITY_ID_PLACEHOLDER,
                    ..Default::default()
                };

                let boxed_record = BoxedGenAIEvalRecord::new(record);
                records.push(ServerRecord::GenAIEval(boxed_record));
            }
        }

        MessageRecord::ServerRecords(ServerRecords::new(records))
    }

    pub fn get_genai_task_results(time_offset: Option<i64>, uid: &str) -> MessageRecord {
        let mut records: Vec<ServerRecord> = Vec::new();
        let offset = time_offset.unwrap_or(0);

        for i in 0..2 {
            for j in 0..25 {
                let record = GenAIEvalTaskResult {
                    record_uid: format!("record_uid_{i}_{j}"),
                    created_at: Utc::now() + chrono::Duration::microseconds(j as i64)
                        - chrono::Duration::days(offset),
                    entity_id: ENTITY_ID_PLACEHOLDER,
                    task_id: format!("task{i}"),
                    task_type: scouter_types::genai::EvaluationTaskType::Assertion,
                    passed: true,
                    value: j as f64,
                    field_path: Some(format!("field.path.{i}")),
                    operator: scouter_types::genai::ComparisonOperator::Contains,
                    expected: Value::Null,
                    actual: Value::Null,
                    message: "All good".to_string(),
                    entity_uid: uid.to_string(),
                    condition: false,
                    stage: i,
                };
                records.push(ServerRecord::GenAITaskRecord(record));
            }
        }

        MessageRecord::ServerRecords(ServerRecords::new(records))
    }

    pub fn get_genai_workflow_results(time_offset: Option<i64>, uid: &str) -> MessageRecord {
        let mut records: Vec<ServerRecord> = Vec::new();
        let offset = time_offset.unwrap_or(0);

        for i in 0..2 {
            for j in 0..25 {
                let record = GenAIEvalWorkflowResult {
                    record_uid: format!("record_uid_{i}_{j}"),
                    created_at: Utc::now() + chrono::Duration::microseconds(j as i64)
                        - chrono::Duration::days(offset),
                    entity_id: ENTITY_ID_PLACEHOLDER,
                    total_tasks: 10,
                    passed_tasks: 8,
                    failed_tasks: 2,
                    pass_rate: 0.8,
                    duration_ms: 1500,
                    entity_uid: uid.to_string(),
                };
                records.push(ServerRecord::GenAIWorkflowRecord(record));
            }
        }

        MessageRecord::ServerRecords(ServerRecords::new(records))
    }

    pub async fn create_genai_drift_profile() -> GenAIEvalProfile {
        let prompt = create_score_prompt(Some(vec!["input".to_string()]));

        let task1 = LLMJudgeTask::new_rs(
            "metric1",
            prompt.clone(),
            Value::Number(4.into()),
            None,
            ComparisonOperator::GreaterThanOrEqual,
            None,
            None,
            None,
            None,
        );

        let task2 = LLMJudgeTask::new_rs(
            "metric2",
            prompt.clone(),
            Value::Number(2.into()),
            None,
            ComparisonOperator::LessThanOrEqual,
            None,
            None,
            None,
            None,
        );

        let tasks = EvaluationTasks::new()
            .add_task(task1)
            .add_task(task2)
            .build();

        let alert_config = GenAIAlertConfig::default();

        let drift_config =
            GenAIDriftConfig::new(SPACE, NAME, VERSION, 1.0, alert_config, None).unwrap();

        GenAIEvalProfile::new(drift_config, tasks).await.unwrap()
    }

    pub async fn insert_alerts(&self) -> Result<(String, String, String), anyhow::Error> {
        let (psi_uid, psi_id) = PostgresClient::create_entity(
            &self.pool,
            "repo_1",
            "model_1",
            "1.0.0",
            DriftType::Psi.to_string(),
        )
        .await?;
        let (custom_uid, custom_id) = PostgresClient::create_entity(
            &self.pool,
            "repo_1",
            "model_1",
            "1.0.0",
            DriftType::Custom.to_string(),
        )
        .await?;
        let (spc_uid, spc_id) = PostgresClient::create_entity(
            &self.pool,
            "repo_1",
            "model_1",
            "1.0.0",
            DriftType::Spc.to_string(),
        )
        .await?;

        // for each entity, insert an 3 alerts
        let entities = vec![
            ("Psi Model Alert", psi_id),
            ("Custom Model Alert", custom_id),
            ("SPC Model Alert", spc_id),
        ];
        for (entity_name, id) in entities {
            for _ in 0..3 {
                let mut alert = BTreeMap::new();
                alert.insert("alert_name".to_string(), entity_name.to_string());
                alert.insert("alert_level".to_string(), "high".to_string());
                PostgresClient::insert_drift_alert(&self.pool, &id, entity_name, &alert).await?;
            }
        }

        Ok((psi_uid, custom_uid, spc_uid))
    }

    pub async fn get_uid_from_args(
        &self,
        space: &str,
        name: &str,
        version: &str,
        drift_type: &DriftType,
    ) -> Result<String, anyhow::Error> {
        let uid = PostgresClient::get_uid_from_args(
            &self.pool,
            space,
            name,
            version,
            drift_type.to_string(),
        )
        .await?;

        Ok(uid)
    }

    pub async fn generate_trace_data(&self) -> Result<(), anyhow::Error> {
        //print current dir
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

    pub async fn create_drift_profile(&self) -> SpcDriftProfile {
        let (array, features) = self.get_data();
        let alert_config = SpcAlertConfig::default();
        let config =
            SpcDriftConfig::new(SPACE, NAME, VERSION, None, None, Some(alert_config), None);

        let monitor = SpcMonitor::new();

        let mut profile = monitor
            .create_2d_drift_profile(&features, &array.view(), &config.unwrap())
            .unwrap();

        let request = profile.create_profile_request().unwrap();

        let body = serde_json::to_string(&request).unwrap();

        let request = Request::builder()
            .uri("/scouter/profile")
            .method("POST")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = self.send_oneshot(request).await;

        //assert response
        assert_eq!(response.status(), StatusCode::OK);
        let registered_response: RegisteredProfileResponse = {
            let val = response.into_body().collect().await.unwrap().to_bytes();
            serde_json::from_slice(&val).unwrap()
        };
        profile.config.uid = registered_response.uid.clone();

        profile
    }

    pub async fn register_drift_profile<T: serde::Serialize>(&self, profile: T) -> String {
        let body = serde_json::to_string(&profile).unwrap();

        let request = Request::builder()
            .uri("/scouter/profile")
            .method("POST")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .unwrap();

        let response = self.send_oneshot(request).await;

        //assert response
        assert_eq!(response.status(), StatusCode::OK);

        let registered_response: RegisteredProfileResponse = {
            let val = response.into_body().collect().await.unwrap().to_bytes();
            serde_json::from_slice(&val).unwrap()
        };

        registered_response.uid
    }

    pub fn populate_genai_records(
        &self,
        uid: &str,
        runtime: &runtime::Runtime,
        time_offset: Option<i64>,
        record_type: RecordType,
    ) {
        let body = match record_type {
            RecordType::GenAITask => {
                let records = TestHelper::get_genai_task_results(time_offset, uid);
                serde_json::to_string(&records).unwrap()
            }
            RecordType::GenAIWorkflow => {
                let records = TestHelper::get_genai_workflow_results(time_offset, uid);
                serde_json::to_string(&records).unwrap()
            }
            RecordType::GenAIEval => {
                let records = TestHelper::get_genai_event_records(time_offset, uid);
                serde_json::to_string(&records).unwrap()
            }
            _ => panic!("Unsupported record type for GenAI task population"),
        };

        let request = Request::builder()
            .uri("/scouter/message")
            .method("POST")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body))
            .unwrap();
        let response = runtime.block_on(async { self.send_oneshot(request).await });
        assert_eq!(response.status(), StatusCode::OK);
    }
}

impl Drop for TestHelper {
    fn drop(&mut self) {
        // Abort the gRPC server if it's still running
        self.grpc_handle.abort();
    }
}

/// Call this at the start of every test to get a clean database
pub async fn setup_test() -> TestHelper {
    // Get the global TestHelper (initialized only once)
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let helper = TestHelper::new(false, false)
        .await
        .expect("Failed to initialize TestHelper");

    // Clean up the database before this test runs
    cleanup_tables(&helper.pool)
        .await
        .expect("Failed to cleanup database before test");

    TestHelper::cleanup_storage();

    helper
}
