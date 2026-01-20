use crate::error::ClientError;
use crate::{
    AuthServiceClient, InsertMessageRequest, InsertMessageResponse, LoginRequest,
    MessageServiceClient, RefreshTokenRequest, ValidateTokenRequest,
};
use scouter_settings::grpc::GrpcConfig;
use std::sync::{Arc, RwLock};
use tonic::metadata::MetadataValue;
use tonic::transport::Channel;
use tonic::Request;
use tonic_health::pb::health_client::HealthClient;
use tonic_health::pb::HealthCheckRequest;
use tracing::{debug, error, info, instrument};

pub const X_REFRESHED_TOKEN: &str = "x-refreshed-token";
pub const AUTHORIZATION: &str = "authorization";

#[derive(Clone, Debug)]
pub struct GrpcClient {
    message_client: MessageServiceClient<Channel>,
    auth_client: AuthServiceClient<Channel>,
    auth_token: Arc<RwLock<String>>,
    config: GrpcConfig,
}

impl GrpcClient {
    pub async fn new(config: GrpcConfig) -> Result<Self, ClientError> {
        let channel = Channel::from_shared(config.server_uri.clone())
            .map_err(|e| {
                error!("Failed to create gRPC channel: {}", e);
                ClientError::GrpcError(e.to_string())
            })?
            .connect()
            .await
            .map_err(|e| {
                error!("Failed to connect to gRPC server: {}", e);
                ClientError::GrpcError(e.to_string())
            })?;

        let message_client = MessageServiceClient::new(channel.clone());
        let auth_client = AuthServiceClient::new(channel);

        let mut grpc_client = Self {
            message_client,
            auth_client,
            auth_token: Arc::new(RwLock::new(String::new())),
            config,
        };

        // Perform initial login via gRPC
        grpc_client.login().await?;

        debug!("gRPC client initialized and authenticated");

        Ok(grpc_client)
    }

    /// Login via gRPC and store the JWT token
    #[instrument(skip_all)]
    pub async fn login(&mut self) -> Result<(), ClientError> {
        debug!("Attempting gRPC login for user: {}", self.config.username);

        let request = Request::new(LoginRequest {
            username: self.config.username.clone(),
            password: self.config.password.clone(),
        });

        let response = self
            .auth_client
            .login(request)
            .await
            .map_err(|e| ClientError::GrpcError(format!("Login failed: {}", e)))?;

        let login_response = response.into_inner();

        if login_response.status != "success" {
            error!("Login failed: {}", login_response.message);
            return Err(ClientError::Unauthorized);
        }

        self.update_token(login_response.token.clone());
        debug!("Successfully logged in via gRPC: {:?}", login_response);

        Ok(())
    }

    /// Refresh token via gRPC
    pub async fn refresh_token(&mut self) -> Result<(), ClientError> {
        debug!("Refreshing token via gRPC");

        let current_token = self.get_current_token();

        let mut request = Request::new(RefreshTokenRequest {
            refresh_token: current_token.clone(),
        });

        // Add current token as bearer token in metadata
        let metadata_value = MetadataValue::try_from(format!("{}", current_token))
            .map_err(|e| ClientError::GrpcError(format!("Invalid metadata: {}", e)))?;

        request.metadata_mut().insert(AUTHORIZATION, metadata_value);

        let response = self
            .auth_client
            .refresh_token(request)
            .await
            .map_err(|e| ClientError::GrpcError(format!("Token refresh failed: {}", e)))?;

        let refresh_response = response.into_inner();

        if refresh_response.status != "success" {
            error!("Token refresh failed: {}", refresh_response.message);
            return Err(ClientError::Unauthorized);
        }

        self.update_token(refresh_response.token);
        info!("Successfully refreshed token via gRPC");

        Ok(())
    }

    /// Validate current token
    pub async fn validate_token(&mut self) -> Result<bool, ClientError> {
        let current_token = self.get_current_token();

        let mut request = Request::new(ValidateTokenRequest {
            token: current_token.clone(),
        });

        let metadata_value = MetadataValue::try_from(format!("{}", current_token))
            .map_err(|e| ClientError::GrpcError(format!("Invalid metadata: {}", e)))?;

        request.metadata_mut().insert(AUTHORIZATION, metadata_value);

        let response = self
            .auth_client
            .validate_token(request)
            .await
            .map_err(|e| ClientError::GrpcError(format!("Token validation failed: {}", e)))?;

        Ok(response.into_inner().is_authenticated)
    }

    fn get_current_token(&self) -> String {
        self.auth_token
            .read()
            .map(|token| token.clone())
            .unwrap_or_default()
    }

    pub fn update_token(&self, token: String) {
        if let Ok(mut token_guard) = self.auth_token.write() {
            *token_guard = token;
        } else {
            error!("Failed to acquire write lock for token update");
        }
    }

    fn create_authenticated_request(
        &self,
        message_record: Vec<u8>,
    ) -> Result<Request<InsertMessageRequest>, ClientError> {
        let mut request = Request::new(InsertMessageRequest { message_record });

        let token = self.get_current_token();
        let metadata_value = MetadataValue::try_from(format!("{}", token))
            .map_err(|e| ClientError::GrpcError(format!("Invalid metadata: {}", e)))?;

        request.metadata_mut().insert(AUTHORIZATION, metadata_value);

        Ok(request)
    }

    /// Insert message with automatic token refresh and retry
    #[instrument(skip_all)]
    pub async fn insert_message(
        &self,
        message_record: Vec<u8>,
    ) -> Result<InsertMessageResponse, ClientError> {
        let request = self.create_authenticated_request(message_record)?;
        let mut client = self.message_client.clone();

        let response = client.insert_message(request).await.map_err(|status| {
            error!(
                "gRPC error (code: {:?}): {}",
                status.code(),
                status.message()
            );
            ClientError::GrpcError(format!(
                "gRPC error: {} (code: {:?})",
                status.message(),
                status.code()
            ))
        })?;

        if let Some(new_token) = response
            .metadata()
            .get(X_REFRESHED_TOKEN)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
        {
            info!("Server refreshed token, updating local copy");
            self.update_token(new_token.to_string());
        }

        Ok(response.into_inner())
    }

    pub async fn health_check(&self) -> Result<bool, ClientError> {
        let channel = Channel::from_shared(self.config.server_uri.clone())
            .map_err(|e| ClientError::GrpcError(format!("Invalid URI: {}", e)))?
            .connect()
            .await
            .map_err(|e| ClientError::GrpcError(format!("Connection failed: {}", e)))?;

        let mut health_client = HealthClient::new(channel);

        // Check health of MessageService
        let request = HealthCheckRequest {
            service: "scouter.grpc.v1.MessageService".to_string(),
        };

        match health_client.check(request).await {
            Ok(response) => {
                let status = response.into_inner().status;
                // Status: 0 = UNKNOWN, 1 = SERVING, 2 = NOT_SERVING
                Ok(status == 1)
            }
            Err(e) => {
                debug!("Health check failed: {}", e);
                Ok(false)
            }
        }
    }
}
