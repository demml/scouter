use crate::error::ClientError;
use crate::{
    AuthServiceClient, DatasetServiceClient, InsertBatchRequest, InsertBatchResponse,
    LoginRequest, QueryDatasetRequest, QueryDatasetResponse, RegisterDatasetRequest,
    RegisterDatasetResponse, RefreshTokenRequest,
};
use scouter_settings::grpc::GrpcConfig;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tonic::metadata::MetadataValue;
use tonic::transport::Channel;
use tonic::Request;
use tracing::{debug, error, info, instrument};

pub const X_REFRESHED_TOKEN: &str = "x-refreshed-token";
pub const AUTHORIZATION: &str = "authorization";

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
        debug!("DatasetGrpcClient initialised and authenticated");
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

        let resp = self
            .dataset_client
            .insert_batch(req)
            .await
            .map_err(|s| {
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

        let resp = self
            .dataset_client
            .query_dataset(req)
            .await
            .map_err(|s| {
                ClientError::GrpcError(format!(
                    "query_dataset failed: {} (code: {:?})",
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
            refresh_token: current.clone(),
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
