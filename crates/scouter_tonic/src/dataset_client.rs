use crate::client::{AUTHORIZATION, X_REFRESHED_TOKEN};
use crate::error::ClientError;
use crate::{
    AuthServiceClient, CancelQueryRequest, CancelQueryResponse, DatasetServiceClient,
    DescribeDatasetRequest, DescribeDatasetResponse, ExecuteQueryRequest, ExecuteQueryResponse,
    ExplainQueryRequest, ExplainQueryResponse, GetTableDetailRequest, GetTableDetailResponse,
    InsertBatchRequest, InsertBatchResponse, ListCatalogsRequest, ListCatalogsResponse,
    ListDatasetsRequest, ListDatasetsResponse, ListSchemasRequest, ListSchemasResponse,
    ListTablesRequest, ListTablesResponse, LoginRequest, PreviewTableRequest, PreviewTableResponse,
    QueryDatasetRequest, QueryDatasetResponse, RefreshTokenRequest, RegisterDatasetRequest,
    RegisterDatasetResponse,
};
use scouter_settings::grpc::GrpcConfig;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tonic::metadata::MetadataValue;
use tonic::transport::Channel;
use tonic::Request;
use tracing::{debug, error, info, instrument};

async fn build_channel(config: &GrpcConfig) -> Result<Channel, ClientError> {
    let mut endpoint = Channel::from_shared(config.server_uri.clone())
        .map_err(|e| ClientError::GrpcError(format!("Invalid URI: {e}")))?;

    if let Some(secs) = config.timeout_secs {
        endpoint = endpoint.timeout(Duration::from_secs(secs));
    }
    if let Some(secs) = config.connect_timeout_secs {
        endpoint = endpoint.connect_timeout(Duration::from_secs(secs));
    }
    if let Some(secs) = config.keep_alive_interval_secs {
        endpoint = endpoint.http2_keep_alive_interval(Duration::from_secs(secs));
    }
    if let Some(secs) = config.keep_alive_timeout_secs {
        endpoint = endpoint.keep_alive_timeout(Duration::from_secs(secs));
    }
    if let Some(enabled) = config.keep_alive_while_idle {
        endpoint = endpoint.keep_alive_while_idle(enabled);
    }

    if config.server_uri.starts_with("https://") {
        endpoint
            .tls_config(tonic::transport::ClientTlsConfig::new().with_native_roots())
            .map_err(|e| ClientError::GrpcError(format!("TLS config failed: {e}")))?
            .connect()
            .await
            .map_err(|e| ClientError::GrpcError(format!("Failed to connect (TLS): {e}")))
    } else {
        endpoint
            .connect()
            .await
            .map_err(|e| ClientError::GrpcError(format!("Failed to connect: {e}")))
    }
}

/// gRPC client for the `DatasetService` RPCs.
///
/// Mirrors the `GrpcClient` pattern in `client.rs` but targets the dataset
/// service. Auth flow: login on connect, bearer token on every request,
/// server-side token refresh via the `x-refreshed-token` response header.
#[derive(Clone, Debug)]
pub struct DatasetGrpcClient {
    dataset_client: DatasetServiceClient<Channel>,
    auth_client: AuthServiceClient<Channel>,
    auth_token: Arc<RwLock<String>>,
    config: GrpcConfig,
}

impl DatasetGrpcClient {
    /// Connect and authenticate. No-network work is performed in [`DatasetClient::new`];
    /// this is called lazily on the first flush or explicit `register()`.
    pub async fn new(config: GrpcConfig) -> Result<Self, ClientError> {
        let channel = build_channel(&config).await.map_err(|e| {
            error!("Failed to connect to gRPC server: {e}");
            e
        })?;

        let dataset_client = DatasetServiceClient::new(channel.clone());
        let auth_client = AuthServiceClient::new(channel);

        let mut client = Self {
            dataset_client,
            auth_client,
            auth_token: Arc::new(RwLock::new(String::new())),
            config,
        };

        client.login().await?;
        debug!("DatasetGrpcClient initialized and authenticated");
        Ok(client)
    }

    #[instrument(skip_all)]
    async fn login(&mut self) -> Result<(), ClientError> {
        let request = Request::new(LoginRequest {
            username: self.config.username.clone(),
            password: self.config.password.clone(),
        });

        let response = self
            .auth_client
            .login(request)
            .await
            .map_err(|e| ClientError::GrpcError(format!("Login failed: {e}")))?
            .into_inner();

        if response.status != "success" {
            error!("Login failed: {}", response.message);
            return Err(ClientError::Unauthorized);
        }

        self.update_token(response.token);
        debug!("DatasetGrpcClient: login successful");
        Ok(())
    }

    fn get_token(&self) -> String {
        self.auth_token
            .read()
            .map(|t| t.clone())
            .unwrap_or_default()
    }

    fn update_token(&self, token: String) {
        if let Ok(mut g) = self.auth_token.write() {
            *g = token;
        } else {
            error!("Failed to acquire write lock for token update");
        }
    }

    fn authenticated_request<T>(&self, inner: T) -> Result<Request<T>, ClientError> {
        let token = self.get_token();
        let meta = MetadataValue::try_from(format!("Bearer {token}"))
            .map_err(|e| ClientError::GrpcError(format!("Invalid metadata: {e}")))?;
        let mut req = Request::new(inner);
        req.metadata_mut().insert(AUTHORIZATION, meta);
        Ok(req)
    }

    fn handle_refreshed_token<T>(&self, resp: &tonic::Response<T>) {
        if let Some(new_token) = resp
            .metadata()
            .get(X_REFRESHED_TOKEN)
            .and_then(|v| v.to_str().ok())
        {
            info!("Server refreshed token, updating local copy");
            self.update_token(new_token.to_string());
        }
    }

    /// Register a dataset. Idempotent: returns `"already_exists"` if the schema
    /// fingerprint matches an existing registration.
    #[instrument(skip_all, fields(table = %table))]
    pub async fn register_dataset(
        &mut self,
        catalog: &str,
        schema_name: &str,
        table: &str,
        json_schema: &str,
        partition_columns: Vec<String>,
    ) -> Result<RegisterDatasetResponse, ClientError> {
        let req = self.authenticated_request(RegisterDatasetRequest {
            catalog: catalog.to_string(),
            schema_name: schema_name.to_string(),
            table: table.to_string(),
            json_schema: json_schema.to_string(),
            partition_columns,
        })?;

        let resp = self
            .dataset_client
            .register_dataset(req)
            .await
            .map_err(|s| {
                ClientError::GrpcError(format!(
                    "register_dataset failed: {} (code: {:?})",
                    s.message(),
                    s.code()
                ))
            })?;

        self.handle_refreshed_token(&resp);
        Ok(resp.into_inner())
    }

    /// Insert a batch of rows as Arrow IPC bytes.
    #[instrument(skip_all, fields(table = %table, fingerprint = %fingerprint))]
    pub async fn insert_batch(
        &mut self,
        catalog: &str,
        schema_name: &str,
        table: &str,
        fingerprint: &str,
        ipc_data: Vec<u8>,
    ) -> Result<InsertBatchResponse, ClientError> {
        let req = self.authenticated_request(InsertBatchRequest {
            catalog: catalog.to_string(),
            schema_name: schema_name.to_string(),
            table: table.to_string(),
            fingerprint: fingerprint.to_string(),
            ipc_data,
        })?;

        let resp = self.dataset_client.insert_batch(req).await.map_err(|s| {
            ClientError::GrpcError(format!(
                "insert_batch failed: {} (code: {:?})",
                s.message(),
                s.code()
            ))
        })?;

        self.handle_refreshed_token(&resp);
        Ok(resp.into_inner())
    }

    /// Execute a SELECT query and return results as Arrow IPC bytes.
    #[instrument(skip_all)]
    pub async fn query_dataset(&mut self, sql: &str) -> Result<QueryDatasetResponse, ClientError> {
        let req = self.authenticated_request(QueryDatasetRequest {
            sql: sql.to_string(),
        })?;

        let resp = self.dataset_client.query_dataset(req).await.map_err(|s| {
            ClientError::GrpcError(format!(
                "query_dataset failed: {} (code: {:?})",
                s.message(),
                s.code()
            ))
        })?;

        self.handle_refreshed_token(&resp);
        Ok(resp.into_inner())
    }

    /// List all registered datasets.
    #[instrument(skip_all)]
    pub async fn list_datasets(&mut self) -> Result<ListDatasetsResponse, ClientError> {
        let req = self.authenticated_request(ListDatasetsRequest {})?;

        let resp = self.dataset_client.list_datasets(req).await.map_err(|s| {
            ClientError::GrpcError(format!(
                "list_datasets failed: {} (code: {:?})",
                s.message(),
                s.code()
            ))
        })?;

        self.handle_refreshed_token(&resp);
        Ok(resp.into_inner())
    }

    /// Describe a specific dataset (metadata + schema).
    #[instrument(skip_all, fields(table = %table))]
    pub async fn describe_dataset(
        &mut self,
        catalog: &str,
        schema_name: &str,
        table: &str,
    ) -> Result<DescribeDatasetResponse, ClientError> {
        let req = self.authenticated_request(DescribeDatasetRequest {
            catalog: catalog.to_string(),
            schema_name: schema_name.to_string(),
            table: table.to_string(),
        })?;

        let resp = self
            .dataset_client
            .describe_dataset(req)
            .await
            .map_err(|s| {
                ClientError::GrpcError(format!(
                    "describe_dataset failed: {} (code: {:?})",
                    s.message(),
                    s.code()
                ))
            })?;

        self.handle_refreshed_token(&resp);
        Ok(resp.into_inner())
    }

    /// List all registered catalogs.
    #[instrument(skip_all)]
    pub async fn list_catalogs(&mut self) -> Result<ListCatalogsResponse, ClientError> {
        let req = self.authenticated_request(ListCatalogsRequest {})?;

        let resp = self.dataset_client.list_catalogs(req).await.map_err(|s| {
            ClientError::GrpcError(format!(
                "list_catalogs failed: {} (code: {:?})",
                s.message(),
                s.code()
            ))
        })?;

        self.handle_refreshed_token(&resp);
        Ok(resp.into_inner())
    }

    /// List schemas within a catalog.
    #[instrument(skip_all, fields(catalog = %catalog))]
    pub async fn list_schemas(
        &mut self,
        catalog: &str,
    ) -> Result<ListSchemasResponse, ClientError> {
        let req = self.authenticated_request(ListSchemasRequest {
            catalog: catalog.to_string(),
        })?;

        let resp = self.dataset_client.list_schemas(req).await.map_err(|s| {
            ClientError::GrpcError(format!(
                "list_schemas failed: {} (code: {:?})",
                s.message(),
                s.code()
            ))
        })?;

        self.handle_refreshed_token(&resp);
        Ok(resp.into_inner())
    }

    /// List tables within a schema.
    #[instrument(skip_all, fields(catalog = %catalog, schema_name = %schema_name))]
    pub async fn list_tables(
        &mut self,
        catalog: &str,
        schema_name: &str,
    ) -> Result<ListTablesResponse, ClientError> {
        let req = self.authenticated_request(ListTablesRequest {
            catalog: catalog.to_string(),
            schema_name: schema_name.to_string(),
        })?;

        let resp = self.dataset_client.list_tables(req).await.map_err(|s| {
            ClientError::GrpcError(format!(
                "list_tables failed: {} (code: {:?})",
                s.message(),
                s.code()
            ))
        })?;

        self.handle_refreshed_token(&resp);
        Ok(resp.into_inner())
    }

    /// Get detailed info for a specific table (schema + stats).
    #[instrument(skip_all, fields(table = %table))]
    pub async fn get_table_detail(
        &mut self,
        catalog: &str,
        schema_name: &str,
        table: &str,
    ) -> Result<GetTableDetailResponse, ClientError> {
        let req = self.authenticated_request(GetTableDetailRequest {
            catalog: catalog.to_string(),
            schema_name: schema_name.to_string(),
            table: table.to_string(),
        })?;

        let resp = self
            .dataset_client
            .get_table_detail(req)
            .await
            .map_err(|s| {
                ClientError::GrpcError(format!(
                    "get_table_detail failed: {} (code: {:?})",
                    s.message(),
                    s.code()
                ))
            })?;

        self.handle_refreshed_token(&resp);
        Ok(resp.into_inner())
    }

    /// Preview rows from a table as Arrow IPC bytes.
    #[instrument(skip_all, fields(table = %table))]
    pub async fn preview_table(
        &mut self,
        catalog: &str,
        schema_name: &str,
        table: &str,
        max_rows: u32,
    ) -> Result<PreviewTableResponse, ClientError> {
        let req = self.authenticated_request(PreviewTableRequest {
            catalog: catalog.to_string(),
            schema_name: schema_name.to_string(),
            table: table.to_string(),
            max_rows,
        })?;

        let resp = self.dataset_client.preview_table(req).await.map_err(|s| {
            ClientError::GrpcError(format!(
                "preview_table failed: {} (code: {:?})",
                s.message(),
                s.code()
            ))
        })?;

        self.handle_refreshed_token(&resp);
        Ok(resp.into_inner())
    }

    /// Execute a SQL query and return results as Arrow IPC bytes with metadata.
    #[instrument(skip_all)]
    pub async fn execute_query(
        &mut self,
        sql: &str,
        query_id: &str,
        max_rows: u32,
    ) -> Result<ExecuteQueryResponse, ClientError> {
        let req = self.authenticated_request(ExecuteQueryRequest {
            sql: sql.to_string(),
            query_id: query_id.to_string(),
            max_rows,
        })?;

        let resp = self.dataset_client.execute_query(req).await.map_err(|s| {
            ClientError::GrpcError(format!(
                "execute_query failed: {} (code: {:?})",
                s.message(),
                s.code()
            ))
        })?;

        self.handle_refreshed_token(&resp);
        Ok(resp.into_inner())
    }

    /// Request cancellation of an in-flight query.
    #[instrument(skip_all, fields(query_id = %query_id))]
    pub async fn cancel_query(
        &mut self,
        query_id: &str,
    ) -> Result<CancelQueryResponse, ClientError> {
        let req = self.authenticated_request(CancelQueryRequest {
            query_id: query_id.to_string(),
        })?;

        let resp = self.dataset_client.cancel_query(req).await.map_err(|s| {
            ClientError::GrpcError(format!(
                "cancel_query failed: {} (code: {:?})",
                s.message(),
                s.code()
            ))
        })?;

        self.handle_refreshed_token(&resp);
        Ok(resp.into_inner())
    }

    /// Explain a SQL query plan (logical + physical).
    #[instrument(skip_all)]
    pub async fn explain_query(
        &mut self,
        sql: &str,
        analyze: bool,
        max_rows: u32,
    ) -> Result<ExplainQueryResponse, ClientError> {
        let req = self.authenticated_request(ExplainQueryRequest {
            sql: sql.to_string(),
            analyze,
            max_rows,
        })?;

        let resp = self.dataset_client.explain_query(req).await.map_err(|s| {
            ClientError::GrpcError(format!(
                "explain_query failed: {} (code: {:?})",
                s.message(),
                s.code()
            ))
        })?;

        self.handle_refreshed_token(&resp);
        Ok(resp.into_inner())
    }

    /// Attempt to refresh the auth token explicitly.
    pub async fn refresh_token(&mut self) -> Result<(), ClientError> {
        let current = self.get_token();
        let mut req = Request::new(RefreshTokenRequest {
            access_token: current.clone(),
        });
        let meta = MetadataValue::try_from(format!("Bearer {current}"))
            .map_err(|e| ClientError::GrpcError(format!("Invalid metadata: {e}")))?;
        req.metadata_mut().insert(AUTHORIZATION, meta);

        let resp = self
            .auth_client
            .refresh_token(req)
            .await
            .map_err(|e| ClientError::GrpcError(format!("Token refresh failed: {e}")))?
            .into_inner();

        if resp.status != "success" {
            return Err(ClientError::Unauthorized);
        }
        self.update_token(resp.token);
        info!("DatasetGrpcClient: token refreshed");
        Ok(())
    }
}
