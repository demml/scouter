use crate::client::{AUTHORIZATION, X_REFRESHED_TOKEN};
use crate::error::ClientError;
use crate::{
    AuthServiceClient, EvalScenarioServiceClient, LoginRequest, RegisterScenariosRequest,
    RegisterScenariosResponse,
};
use scouter_settings::grpc::GrpcConfig;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tonic::metadata::MetadataValue;
use tonic::transport::Channel;
use tonic::Request;
use tracing::{debug, error, instrument, warn};

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
        warn!("Connecting to gRPC server without TLS — use https:// in production");
        endpoint
            .connect()
            .await
            .map_err(|e| ClientError::GrpcError(format!("Failed to connect: {e}")))
    }
}

#[derive(Clone, Debug)]
pub struct EvalScenarioGrpcClient {
    eval_client: EvalScenarioServiceClient<Channel>,
    auth_client: AuthServiceClient<Channel>,
    auth_token: Arc<RwLock<String>>,
    config: GrpcConfig,
}

impl EvalScenarioGrpcClient {
    pub async fn new(config: GrpcConfig) -> Result<Self, ClientError> {
        let channel = build_channel(&config).await.map_err(|e| {
            error!("Failed to connect to gRPC server: {e}");
            e
        })?;

        let eval_client = EvalScenarioServiceClient::new(channel.clone());
        let auth_client = AuthServiceClient::new(channel);

        let mut client = Self {
            eval_client,
            auth_client,
            auth_token: Arc::new(RwLock::new(String::new())),
            config,
        };

        client.login().await?;
        debug!("EvalScenarioGrpcClient initialized and authenticated");
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
        Ok(())
    }

    fn get_token(&self) -> String {
        self.auth_token
            .read()
            .map(|g| g.clone())
            .unwrap_or_default()
    }

    fn update_token(&self, token: String) {
        if let Ok(mut guard) = self.auth_token.write() {
            *guard = token;
        } else {
            error!("Failed to acquire write lock for token update");
        }
    }

    fn handle_refreshed_token(&self, headers: &tonic::metadata::MetadataMap) {
        if let Some(new_token) = headers.get(X_REFRESHED_TOKEN) {
            if let Ok(token_str) = new_token.to_str() {
                self.update_token(token_str.to_string());
            }
        }
    }

    fn authenticated_request<T>(&self, inner: T) -> Result<Request<T>, ClientError> {
        let mut request = Request::new(inner);
        let token = self.get_token();
        let header_value = MetadataValue::try_from(format!("Bearer {token}"))
            .map_err(|e| ClientError::GrpcError(format!("Invalid token: {e}")))?;
        request.metadata_mut().insert(AUTHORIZATION, header_value);
        Ok(request)
    }

    #[instrument(skip_all)]
    pub async fn register_scenarios(
        &mut self,
        collection_id: String,
        scenarios_json: String,
    ) -> Result<RegisterScenariosResponse, ClientError> {
        let request = self.authenticated_request(RegisterScenariosRequest {
            collection_id,
            scenarios_json,
        })?;

        let response = self
            .eval_client
            .register_scenarios(request)
            .await
            .map_err(|e| ClientError::GrpcError(format!("RegisterScenarios failed: {e}")))?;

        self.handle_refreshed_token(response.metadata());
        Ok(response.into_inner())
    }
}
