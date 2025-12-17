use crate::error::ClientError;
use crate::{InsertMessageRequest, InsertMessageResponse, MessageServiceClient};
use scouter_http::HttpClient;
use scouter_settings::http::HttpConfig;
use tonic::metadata::MetadataValue;
use tonic::transport::Channel;
use tonic::Request;
use tracing::{error, info, warn};

pub const X_REFRESHED_TOKEN: &str = "x-refreshed-token";
pub const AUTHORIZATION: &str = "authorization";

#[derive(Clone)]
pub struct GrpcClient {
    client: MessageServiceClient<Channel>,
    http_client: HttpClient,
}

impl GrpcClient {
    pub async fn new(config: HttpConfig) -> Result<Self, ClientError> {
        // Use the configured gRPC port or default to 50051
        let grpc_port = std::env::var("SCOUTER_GRPC_PORT").unwrap_or_else(|_| "50051".to_string());
        let grpc_uri = format!(
            "http://{}:{}",
            config.server_uri.split(':').next().unwrap_or("localhost"),
            grpc_port
        );

        let channel = Channel::from_shared(grpc_uri.clone())
            .map_err(|e| ClientError::GrpcError(e.to_string()))?
            .connect()
            .await
            .map_err(|e| ClientError::GrpcError(e.to_string()))?;

        let client = MessageServiceClient::new(channel);

        let grpc_client = Self {
            client,
            http_client: HttpClient::new(config.clone())?,
        };

        grpc_client.refresh_token()?;

        info!("gRPC client initialized and authenticated");

        Ok(grpc_client)
    }

    fn refresh_token(&self) -> Result<(), ClientError> {
        self.http_client.refresh_token()?;
        Ok(())
    }

    fn get_current_token(&self) -> String {
        self.http_client.get_current_token()
    }

    pub fn update_token(&self, token: String) {
        if let Ok(mut token_guard) = self.http_client.auth_token.write() {
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
        let metadata_value = MetadataValue::try_from(format!("Bearer {}", token))
            .map_err(|e| ClientError::GrpcError(format!("Invalid metadata: {}", e)))?;

        request.metadata_mut().insert(AUTHORIZATION, metadata_value);

        Ok(request)
    }

    /// Insert message with automatic token refresh and retry
    ///
    /// This method handles token expiration automatically by:
    /// 1. Attempting the request with the current token
    /// 2. Checking for server-side token refresh (via x-refreshed-token header)
    /// 3. On authentication failure, refreshing the token and retrying once
    pub async fn insert_message(
        &mut self,
        message_record: Vec<u8>,
    ) -> Result<InsertMessageResponse, ClientError> {
        let request = self.create_authenticated_request(message_record.clone())?;

        match self.client.insert_message(request).await {
            Ok(response) => {
                // Check if server refreshed the token for us
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
            Err(status) => {
                // Handle authentication errors with automatic retry
                if status.code() == tonic::Code::Unauthenticated {
                    warn!(
                        "Authentication failed: {}. Attempting token refresh and retry",
                        status.message()
                    );

                    // Refresh token via HTTP endpoint
                    self.refresh_token()?;

                    // Retry request with new token
                    let retry_request = self.create_authenticated_request(message_record)?;

                    let response =
                        self.client
                            .insert_message(retry_request)
                            .await
                            .map_err(|e| {
                                error!("Retry after token refresh failed: {:?}", e);
                                ClientError::GrpcError(e.to_string())
                            })?;

                    info!("Request succeeded after token refresh");
                    Ok(response.into_inner())
                } else {
                    error!(
                        "gRPC error (code: {:?}): {}",
                        status.code(),
                        status.message()
                    );
                    Err(ClientError::GrpcError(format!(
                        "gRPC error: {} (code: {:?})",
                        status.message(),
                        status.code()
                    )))
                }
            }
        }
    }
}
