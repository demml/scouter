use crate::producer::http::types::{HTTPConfig, JwtToken, RequestType, Routes};

use scouter_error::ScouterError;

use reqwest::Response;
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client,
};
use scouter_types::ServerRecords;
use serde_json::Value;
use tracing::debug;

const TIMEOUT_SECS: u64 = 30;
const REDACTED: &str = "REDACTED";

/// Create a new HTTP client that can be shared across different clients
pub fn build_http_client() -> Result<Client, ScouterError> {
    let client_builder = Client::builder().timeout(std::time::Duration::from_secs(TIMEOUT_SECS));
    let client = client_builder
        .build()
        .map_err(|e| ScouterError::Error(format!("Failed to create client with error: {}", e)))?;
    Ok(client)
}

#[derive(Debug, Clone)]
pub struct HTTPClient {
    client: Client,
    config: HTTPConfig,
}

impl HTTPClient {
    pub async fn new(config: HTTPConfig) -> Result<Self, ScouterError> {
        let client = build_http_client()?;

        let mut api_client = HTTPClient { client, config };

        if api_client.config.use_auth {
            api_client.get_jwt_token().await?;

            // mask the username and password
            api_client.config.username = REDACTED.to_string();
            api_client.config.password = REDACTED.to_string();

            // mask the env variables
            std::env::set_var("SCOUTER_USERNAME", REDACTED);
            std::env::set_var("SCOUTER_PASSWORD", REDACTED);
        }

        Ok(api_client)
    }

    async fn get_jwt_token(&mut self) -> Result<(), ScouterError> {
        if !self.config.use_auth {
            return Ok(());
        }

        let mut headers = HeaderMap::new();
        headers.insert(
            "Username",
            HeaderValue::from_str(&self.config.username).map_err(|e| {
                ScouterError::Error(format!("Failed to create header with error: {}", e))
            })?,
        );

        headers.insert(
            "Password",
            HeaderValue::from_str(&self.config.password).map_err(|e| {
                ScouterError::Error(format!("Failed to create header with error: {}", e))
            })?,
        );

        let url = format!(
            "{}/{}",
            self.config.server_url,
            Routes::AuthApiLogin.as_str()
        );
        let response = self
            .client
            .get(url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| ScouterError::Error(format!("Failed to send request with error: {}", e)))?
            .json::<JwtToken>()
            .await
            .map_err(|e| {
                ScouterError::Error(format!("Failed to parse response with error: {}", e))
            })?;

        self.config.auth_token = response.token;

        Ok(())
    }

    /// Refresh the JWT token when it expires
    /// This function is called with the old JWT token, which is then verified with the server refresh token
    async fn refresh_token(&mut self) -> Result<(), ScouterError> {
        if !self.config.use_auth {
            return Ok(());
        }

        let url = format!(
            "{}/{}",
            self.config.server_url,
            Routes::AuthApiRefresh.as_str()
        );
        let response = self
            .client
            .get(url)
            .bearer_auth(&self.config.auth_token)
            .send()
            .await
            .map_err(|e| ScouterError::Error(format!("Failed to send request with error: {}", e)))?
            .json::<JwtToken>()
            .await
            .map_err(|e| {
                ScouterError::Error(format!("Failed to parse response with error: {}", e))
            })?;

        self.config.auth_token = response.token;

        Ok(())
    }

    async fn request(
        self,
        route: Routes,
        request_type: RequestType,
        body_params: Option<Value>,
        query_string: Option<String>,
        headers: Option<HeaderMap>,
    ) -> Result<Response, ScouterError> {
        let headers = headers.unwrap_or_default();

        let url = format!("{}/{}", self.config.server_url, route.as_str());
        let response = match request_type {
            RequestType::Get => {
                let url = if let Some(query_string) = query_string {
                    format!("{}?{}", url, query_string)
                } else {
                    url
                };

                self.client
                    .get(url)
                    .headers(headers)
                    .bearer_auth(&self.config.auth_token)
                    .send()
                    .await
                    .map_err(|e| {
                        ScouterError::Error(format!("Failed to send request with error: {}", e))
                    })?
            }
            RequestType::Post => self
                .client
                .post(url)
                .headers(headers)
                .json(&body_params)
                .bearer_auth(self.config.auth_token)
                .send()
                .await
                .map_err(|e| {
                    ScouterError::Error(format!("Failed to send request with error: {}", e))
                })?,
        };

        Ok(response)
    }

    pub async fn request_with_retry(
        &mut self,
        route: Routes,
        request_type: RequestType,
        body_params: Option<Value>,
        query_params: Option<String>,
        headers: Option<HeaderMap>,
    ) -> Result<Response, ScouterError> {
        // this will attempt to send a request. If the request fails, it will refresh the token and try again. If it fails all 3 times it will return an error
        let mut attempts = 0;
        let max_attempts = 3;
        let mut response: Result<Response, ScouterError>;

        loop {
            attempts += 1;

            let client = self.clone();
            response = client
                .request(
                    route.clone(),
                    request_type.clone(),
                    body_params.clone(),
                    query_params.clone(),
                    headers.clone(),
                )
                .await;

            if response.is_ok() || attempts >= max_attempts {
                break;
            }

            if response.is_err() {
                self.refresh_token().await.map_err(|e| {
                    ScouterError::Error(format!("Failed to refresh token with error: {}", e))
                })?;
            }
        }

        let response = response.map_err(|e| {
            ScouterError::Error(format!("Failed to send request with error: {}", e))
        })?;

        Ok(response)
    }
}

#[derive(Debug, Clone)]
pub struct HTTPProducer {
    client: HTTPClient,
}

impl HTTPProducer {
    pub async fn new(config: HTTPConfig) -> Result<Self, ScouterError> {
        let client = HTTPClient::new(config).await?;
        Ok(HTTPProducer { client })
    }

    pub async fn publish(&mut self, message: ServerRecords) -> Result<(), ScouterError> {
        let serialized_msg: Value = serde_json::to_value(&message).map_err(|e| {
            ScouterError::Error(format!("Failed to serialize message with error: {}", e))
        })?;
        let response = self
            .client
            .request_with_retry(
                Routes::Drift,
                RequestType::Post,
                Some(serialized_msg),
                None,
                None,
            )
            .await?;

        debug!("Published message to drift with response: {:?}", response);

        Ok(())
    }

    pub fn flush(&self) -> Result<(), ScouterError> {
        Ok(())
    }
}
