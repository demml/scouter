use pyo3::prelude::*;

#[cfg(feature = "server")]
use scouter_auth::auth::AuthManager;
#[cfg(feature = "server")]
use scouter_dataframe::error::DatasetEngineError;
#[cfg(feature = "server")]
use scouter_dataframe::parquet::bifrost::ipc::{batches_to_ipc_bytes, ipc_bytes_to_batches};
#[cfg(feature = "server")]
use scouter_dataframe::parquet::bifrost::manager::DatasetEngineManager;
#[cfg(feature = "server")]
use scouter_dataframe::parquet::bifrost::registry::RegistrationResult;
#[cfg(feature = "server")]
use scouter_settings::ObjectStorageSettings;
#[cfg(feature = "server")]
use scouter_sql::sql::schema::User;
#[cfg(feature = "server")]
use scouter_tonic::{
    AuthService, AuthServiceServer, CancelQueryRequest, CancelQueryResponse, DatasetInfo,
    DatasetService, DatasetServiceServer, DescribeDatasetRequest, DescribeDatasetResponse,
    ExecuteQueryRequest, ExecuteQueryResponse, ExplainQueryRequest, ExplainQueryResponse,
    GetTableDetailRequest, GetTableDetailResponse, InsertBatchRequest, InsertBatchResponse,
    ListCatalogsRequest, ListCatalogsResponse, ListDatasetsRequest, ListDatasetsResponse,
    ListSchemasRequest, ListSchemasResponse, ListTablesRequest, ListTablesResponse, LoginRequest,
    LoginResponse, PreviewTableRequest, PreviewTableResponse, QueryDatasetRequest,
    QueryDatasetResponse, RefreshTokenRequest, RefreshTokenResponse, RegisterDatasetRequest,
    RegisterDatasetResponse, ValidateTokenRequest, ValidateTokenResponse,
};
#[cfg(feature = "server")]
use scouter_types::dataset::schema::{fingerprint_from_json_schema, inject_system_columns};
#[cfg(feature = "server")]
use scouter_types::dataset::{DatasetFingerprint, DatasetNamespace, DatasetRegistration};
#[cfg(feature = "server")]
use scouter_types::StorageType;
#[cfg(feature = "server")]
use std::net::TcpListener as StdTcpListener;
#[cfg(feature = "server")]
use std::sync::Arc;
#[cfg(feature = "server")]
use std::thread::sleep;
#[cfg(feature = "server")]
use std::time::Duration;
use thiserror::Error;
#[cfg(feature = "server")]
use tonic::transport::{Channel, Server};
#[cfg(feature = "server")]
use tonic::{Request, Response, Status};
#[cfg(feature = "server")]
use tonic_health::server::health_reporter;
#[cfg(feature = "server")]
use tracing::error;
use tracing::instrument;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Error, Debug)]
pub enum DatasetServerError {
    #[error("Failed to find free port")]
    PortError,
    #[error("Server failed to start: {0}")]
    StartError(String),
    #[error("Server feature not enabled")]
    FeatureNotEnabled,
}

impl From<DatasetServerError> for PyErr {
    fn from(e: DatasetServerError) -> PyErr {
        pyo3::exceptions::PyRuntimeError::new_err(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// Server-only: PassthroughAuthService
// ---------------------------------------------------------------------------

#[cfg(feature = "server")]
const TEST_JWT_SECRET: &str = "scouter-dataset-test-jwt-secret!!";
#[cfg(feature = "server")]
const TEST_REFRESH_SECRET: &str = "scouter-dataset-test-refresh-key!";
#[cfg(feature = "server")]
const MAX_MESSAGE_SIZE: usize = 64 * 1024 * 1024;

#[cfg(feature = "server")]
pub struct PassthroughAuthService {
    auth_manager: AuthManager,
}

#[cfg(feature = "server")]
impl PassthroughAuthService {
    fn new() -> Self {
        Self {
            auth_manager: AuthManager::new(TEST_JWT_SECRET, TEST_REFRESH_SECRET),
        }
    }

    fn into_service(self) -> AuthServiceServer<Self> {
        AuthServiceServer::new(self)
    }

    fn make_user(username: &str) -> User {
        User::new(
            username.to_string(),
            "unused".to_string(),
            "test@test.com".to_string(),
            vec![],
            Some(vec!["read:all".to_string(), "write:all".to_string()]),
            Some(vec!["admin".to_string()]),
            Some("admin".to_string()),
            None,
        )
    }
}

#[cfg(feature = "server")]
#[tonic::async_trait]
impl AuthService for PassthroughAuthService {
    async fn login(
        &self,
        request: Request<LoginRequest>,
    ) -> Result<Response<LoginResponse>, Status> {
        let req = request.into_inner();
        let user = Self::make_user(&req.username);
        let token = self.auth_manager.generate_jwt(&user);
        Ok(Response::new(LoginResponse {
            token,
            status: "success".to_string(),
            message: "Login successful".to_string(),
        }))
    }

    async fn refresh_token(
        &self,
        request: Request<RefreshTokenRequest>,
    ) -> Result<Response<RefreshTokenResponse>, Status> {
        let req = request.into_inner();
        let claims = self
            .auth_manager
            .decode_jwt_without_validation(&req.access_token)
            .map_err(|_| Status::unauthenticated("Invalid token"))?;
        let user = Self::make_user(&claims.sub);
        let token = self.auth_manager.generate_jwt(&user);
        Ok(Response::new(RefreshTokenResponse {
            token,
            status: "success".to_string(),
            message: "Token refreshed".to_string(),
        }))
    }

    async fn validate_token(
        &self,
        request: Request<ValidateTokenRequest>,
    ) -> Result<Response<ValidateTokenResponse>, Status> {
        let req = request.into_inner();
        let is_valid = self.auth_manager.validate_jwt(&req.token).is_ok();
        Ok(Response::new(ValidateTokenResponse {
            is_authenticated: is_valid,
            status: if is_valid { "success" } else { "failed" }.to_string(),
            message: if is_valid {
                "Token valid".to_string()
            } else {
                "Token invalid".to_string()
            },
        }))
    }
}

// ---------------------------------------------------------------------------
// Server-only: MockDatasetGrpcService
// ---------------------------------------------------------------------------

#[cfg(feature = "server")]
#[derive(Clone)]
pub struct MockDatasetGrpcService {
    manager: Arc<DatasetEngineManager>,
}

#[cfg(feature = "server")]
impl MockDatasetGrpcService {
    fn new(manager: Arc<DatasetEngineManager>) -> Self {
        Self { manager }
    }

    fn into_server(self) -> DatasetServiceServer<Self> {
        DatasetServiceServer::new(self)
            .max_decoding_message_size(MAX_MESSAGE_SIZE)
            .max_encoding_message_size(MAX_MESSAGE_SIZE)
    }
}

#[cfg(feature = "server")]
fn map_dataset_error(e: DatasetEngineError) -> Status {
    match &e {
        DatasetEngineError::TableNotFound(_) => Status::not_found(e.to_string()),
        DatasetEngineError::FingerprintMismatch { .. } => {
            Status::failed_precondition(e.to_string())
        }
        DatasetEngineError::ChannelClosed => Status::unavailable(e.to_string()),
        DatasetEngineError::DatasetError(_) => Status::invalid_argument(e.to_string()),
        DatasetEngineError::SqlValidationError(_) => Status::invalid_argument(e.to_string()),
        _ => {
            error!("Dataset engine error: {:?}", e);
            Status::internal("Internal server error".to_string())
        }
    }
}

#[cfg(feature = "server")]
#[tonic::async_trait]
impl DatasetService for MockDatasetGrpcService {
    #[instrument(skip_all)]
    async fn register_dataset(
        &self,
        request: Request<RegisterDatasetRequest>,
    ) -> Result<Response<RegisterDatasetResponse>, Status> {
        let req = request.into_inner();

        let namespace = DatasetNamespace::new(&req.catalog, &req.schema_name, &req.table)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let arrow_schema =
            scouter_types::dataset::schema::json_schema_to_arrow(&req.json_schema)
                .map_err(|e| Status::invalid_argument(format!("Invalid JSON schema: {e}")))?;

        let arrow_schema_with_sys = inject_system_columns(arrow_schema).map_err(|e| {
            Status::invalid_argument(format!("Failed to inject system columns: {e}"))
        })?;

        let arrow_schema_json = serde_json::to_string(&arrow_schema_with_sys)
            .map_err(|e| Status::internal(format!("Failed to serialize Arrow schema: {e}")))?;

        let fingerprint = fingerprint_from_json_schema(&req.json_schema)
            .map_err(|e| Status::invalid_argument(format!("Failed to compute fingerprint: {e}")))?;

        let registration = DatasetRegistration::new(
            namespace,
            fingerprint.clone(),
            arrow_schema_json,
            req.json_schema,
            req.partition_columns,
        );

        let result = self
            .manager
            .register_dataset(&registration)
            .await
            .map_err(map_dataset_error)?;

        let status = match result {
            RegistrationResult::Created => "created",
            RegistrationResult::AlreadyExists => "already_exists",
        };

        Ok(Response::new(RegisterDatasetResponse {
            status: status.to_string(),
            fingerprint: fingerprint.as_str().to_string(),
        }))
    }

    #[instrument(skip_all)]
    async fn insert_batch(
        &self,
        request: Request<InsertBatchRequest>,
    ) -> Result<Response<InsertBatchResponse>, Status> {
        let req = request.into_inner();

        let namespace = DatasetNamespace::new(&req.catalog, &req.schema_name, &req.table)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let fingerprint = DatasetFingerprint(req.fingerprint);

        let batches = ipc_bytes_to_batches(&req.ipc_data)
            .map_err(|e| Status::invalid_argument(format!("Invalid IPC data: {e}")))?;

        let mut total_rows: u64 = 0;
        for batch in batches {
            total_rows += batch.num_rows() as u64;
            self.manager
                .insert_batch(&namespace, &fingerprint, batch)
                .await
                .map_err(map_dataset_error)?;
        }

        Ok(Response::new(InsertBatchResponse {
            rows_accepted: total_rows,
        }))
    }

    #[instrument(skip_all)]
    async fn query_dataset(
        &self,
        request: Request<QueryDatasetRequest>,
    ) -> Result<Response<QueryDatasetResponse>, Status> {
        let req = request.into_inner();

        let batches = self
            .manager
            .query(&req.sql)
            .await
            .map_err(map_dataset_error)?;

        let ipc_data = batches_to_ipc_bytes(&batches)
            .map_err(|e| Status::internal(format!("Failed to serialize query results: {e}")))?;

        Ok(Response::new(QueryDatasetResponse { ipc_data }))
    }

    #[instrument(skip_all)]
    async fn list_datasets(
        &self,
        _request: Request<ListDatasetsRequest>,
    ) -> Result<Response<ListDatasetsResponse>, Status> {
        let datasets = self
            .manager
            .list_datasets()
            .into_iter()
            .map(|r| DatasetInfo {
                catalog: r.namespace.catalog,
                schema_name: r.namespace.schema_name,
                table: r.namespace.table,
                fingerprint: r.fingerprint.as_str().to_string(),
                partition_columns: r.partition_columns,
                status: r.status.to_string(),
                created_at: r.created_at.to_rfc3339(),
                updated_at: r.updated_at.to_rfc3339(),
            })
            .collect();

        Ok(Response::new(ListDatasetsResponse { datasets }))
    }

    #[instrument(skip_all)]
    async fn describe_dataset(
        &self,
        request: Request<DescribeDatasetRequest>,
    ) -> Result<Response<DescribeDatasetResponse>, Status> {
        let req = request.into_inner();

        let namespace = DatasetNamespace::new(&req.catalog, &req.schema_name, &req.table)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let registration = self
            .manager
            .get_dataset_info(&namespace)
            .ok_or_else(|| Status::not_found(format!("Dataset not found: {}", namespace.fqn())))?;

        let info = DatasetInfo {
            catalog: registration.namespace.catalog,
            schema_name: registration.namespace.schema_name,
            table: registration.namespace.table,
            fingerprint: registration.fingerprint.as_str().to_string(),
            partition_columns: registration.partition_columns,
            status: registration.status.to_string(),
            created_at: registration.created_at.to_rfc3339(),
            updated_at: registration.updated_at.to_rfc3339(),
        };

        Ok(Response::new(DescribeDatasetResponse {
            info: Some(info),
            arrow_schema_json: registration.arrow_schema_json,
        }))
    }

    async fn list_catalogs(
        &self,
        _request: Request<ListCatalogsRequest>,
    ) -> Result<Response<ListCatalogsResponse>, Status> {
        Err(Status::unimplemented("Not implemented in mock"))
    }

    async fn list_schemas(
        &self,
        _request: Request<ListSchemasRequest>,
    ) -> Result<Response<ListSchemasResponse>, Status> {
        Err(Status::unimplemented("Not implemented in mock"))
    }

    async fn list_tables(
        &self,
        _request: Request<ListTablesRequest>,
    ) -> Result<Response<ListTablesResponse>, Status> {
        Err(Status::unimplemented("Not implemented in mock"))
    }

    async fn get_table_detail(
        &self,
        _request: Request<GetTableDetailRequest>,
    ) -> Result<Response<GetTableDetailResponse>, Status> {
        Err(Status::unimplemented("Not implemented in mock"))
    }

    async fn preview_table(
        &self,
        _request: Request<PreviewTableRequest>,
    ) -> Result<Response<PreviewTableResponse>, Status> {
        Err(Status::unimplemented("Not implemented in mock"))
    }

    async fn execute_query(
        &self,
        _request: Request<ExecuteQueryRequest>,
    ) -> Result<Response<ExecuteQueryResponse>, Status> {
        Err(Status::unimplemented("Not implemented in mock"))
    }

    async fn cancel_query(
        &self,
        _request: Request<CancelQueryRequest>,
    ) -> Result<Response<CancelQueryResponse>, Status> {
        Err(Status::unimplemented("Not implemented in mock"))
    }

    async fn explain_query(
        &self,
        _request: Request<ExplainQueryRequest>,
    ) -> Result<Response<ExplainQueryResponse>, Status> {
        Err(Status::unimplemented("Not implemented in mock"))
    }
}

// ---------------------------------------------------------------------------
// BifrostTestServer
// ---------------------------------------------------------------------------

#[pyclass(skip_from_py_object)]
#[allow(dead_code)]
pub struct BifrostTestServer {
    #[cfg(feature = "server")]
    runtime: Arc<tokio::runtime::Runtime>,
    #[cfg(feature = "server")]
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    #[cfg(feature = "server")]
    storage_dir: Option<tempfile::TempDir>,
    cleanup: bool,
}

#[pymethods]
impl BifrostTestServer {
    #[new]
    #[pyo3(signature = (cleanup = true))]
    fn new(cleanup: bool) -> Self {
        Self {
            #[cfg(feature = "server")]
            runtime: Arc::new(tokio::runtime::Runtime::new().unwrap()),
            #[cfg(feature = "server")]
            shutdown_tx: None,
            #[cfg(feature = "server")]
            storage_dir: None,
            cleanup,
        }
    }

    #[instrument(name = "start_bifrost_server", skip_all)]
    fn start_server(&mut self) -> Result<(), DatasetServerError> {
        #[cfg(feature = "server")]
        {
            let dir =
                tempfile::tempdir().map_err(|e| DatasetServerError::StartError(e.to_string()))?;
            let storage_path = dir.path().to_path_buf();
            self.storage_dir = Some(dir);

            let grpc_port = (50061..50071)
                .find(|port| StdTcpListener::bind(("127.0.0.1", *port)).is_ok())
                .ok_or(DatasetServerError::PortError)?;

            unsafe {
                std::env::set_var("SCOUTER_GRPC_URI", format!("http://127.0.0.1:{grpc_port}"));
                std::env::set_var("SCOUTER_STORAGE_URI", storage_path.to_str().unwrap());
                std::env::set_var("APP_ENV", "dev_client");
            }

            let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
            self.shutdown_tx = Some(shutdown_tx);

            let runtime = self.runtime.clone();
            runtime.spawn(async move {
                let storage_settings = ObjectStorageSettings {
                    storage_uri: storage_path.to_str().unwrap().to_string(),
                    storage_type: StorageType::Local,
                    region: "us-east-1".to_string(),
                    trace_compaction_interval_hours: 999,
                    trace_flush_interval_secs: 2,
                    trace_refresh_interval_secs: 2,
                };

                let manager = match DatasetEngineManager::with_config(
                    &storage_settings,
                    30, // engine_ttl_secs
                    50, // max_active_engines
                    2,  // flush_interval_secs — fast flush for tests
                    50, // max_buffer_rows — trigger flush at 50 rows for tests
                    2,  // refresh_interval_secs
                )
                .await
                {
                    Ok(m) => Arc::new(m),
                    Err(e) => {
                        error!("Failed to create DatasetEngineManager: {}", e);
                        return;
                    }
                };

                let (health_reporter, health_service) = health_reporter();
                let dataset_service = MockDatasetGrpcService::new(manager).into_server();
                let auth_service = PassthroughAuthService::new().into_service();

                health_reporter
                    .set_serving::<DatasetServiceServer<MockDatasetGrpcService>>()
                    .await;
                health_reporter
                    .set_serving::<AuthServiceServer<PassthroughAuthService>>()
                    .await;

                let addr = format!("0.0.0.0:{grpc_port}").parse().unwrap();
                if let Err(e) = Server::builder()
                    .add_service(health_service)
                    .add_service(auth_service)
                    .add_service(dataset_service)
                    .serve_with_shutdown(addr, async {
                        shutdown_rx.await.ok();
                    })
                    .await
                {
                    error!("Dataset gRPC server error: {}", e);
                }
            });

            let runtime_clone = self.runtime.clone();
            let mut attempts = 0;
            let max_attempts = 50;
            loop {
                let ready = runtime_clone.block_on(async {
                    let channel = Channel::from_shared(format!("http://127.0.0.1:{grpc_port}"))
                        .ok()?
                        .connect()
                        .await
                        .ok()?;
                    let mut hc = tonic_health::pb::health_client::HealthClient::new(channel);
                    let resp = hc
                        .check(tonic_health::pb::HealthCheckRequest {
                            service: "scouter.grpc.v1.DatasetService".to_string(),
                        })
                        .await
                        .ok()?;
                    Some(resp.into_inner().status == 1)
                });

                if ready == Some(true) {
                    println!("✅ BifrostTestServer ready on gRPC port {grpc_port}");
                    return Ok(());
                }

                attempts += 1;
                if attempts >= max_attempts {
                    return Err(DatasetServerError::StartError(
                        "Dataset gRPC server failed to become ready".to_string(),
                    ));
                }
                sleep(Duration::from_millis(100 + attempts * 20));
            }
        }
        #[cfg(not(feature = "server"))]
        {
            Err(DatasetServerError::FeatureNotEnabled)
        }
    }

    fn stop_server(&mut self) -> Result<(), DatasetServerError> {
        #[cfg(feature = "server")]
        {
            if let Some(tx) = self.shutdown_tx.take() {
                let _ = tx.send(());
            }

            unsafe {
                std::env::remove_var("SCOUTER_GRPC_URI");
                std::env::remove_var("SCOUTER_STORAGE_URI");
                std::env::remove_var("APP_ENV");
            }

            Ok(())
        }
        #[cfg(not(feature = "server"))]
        {
            Err(DatasetServerError::FeatureNotEnabled)
        }
    }

    fn __enter__(mut self_: PyRefMut<Self>) -> Result<PyRefMut<Self>, DatasetServerError> {
        self_.start_server()?;
        Ok(self_)
    }

    fn __exit__(
        &mut self,
        _exc_type: Py<PyAny>,
        _exc_value: Py<PyAny>,
        _traceback: Py<PyAny>,
    ) -> Result<(), DatasetServerError> {
        self.stop_server()
    }
}
