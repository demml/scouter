use crate::producer::http::types::{HTTPConfig, JwtToken, RequestType, Routes};

use reqwest::header;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client, Response};
use scouter_error::ScouterError;
use scouter_types::ServerRecords;
use serde_json::Value;
use tracing::{debug, error, instrument};

const TIMEOUT_SECS: u64 = 60;

/// Create a new HTTP client that can be shared across different clients

/// Create a new HTTP client that can be shared across different clients
pub fn build_http_client(settings: &HTTPConfig) -> Result<Client, ScouterError> {
    let mut headers = HeaderMap::new();

    headers.insert(
        "Username",
        HeaderValue::from_str(&settings.username).map_err(|e| {
            ScouterError::Error(format!("Failed to create header with error: {}", e))
        })?,
    );

    headers.insert(
        "Password",
        HeaderValue::from_str(&settings.password).map_err(|e| {
            ScouterError::Error(format!("Failed to create header with error: {}", e))
        })?,
    );

    let client_builder = Client::builder().timeout(std::time::Duration::from_secs(TIMEOUT_SECS));
    let client = client_builder
        .default_headers(headers)
        .build()
        .map_err(|e| ScouterError::Error(format!("Failed to create client with error: {}", e)))?;
    Ok(client)
}

#[derive(Debug, Clone)]
pub struct HTTPClient {
    client: Client,
    config: HTTPConfig,
    base_path: String,
}

impl HTTPClient {
    pub async fn new(config: HTTPConfig) -> Result<Self, ScouterError> {
        let client = build_http_client(&config)?;

        let mut api_client = HTTPClient {
            client,
            config: config.clone(),
            base_path: format!("{}/{}", config.server_url, "scouter"),
        };

        api_client.get_jwt_token().await.map_err(|e| {
            error!("Failed to get JWT token: {}", e);
            ScouterError::Error(format!("Failed to get JWT token with error: {}", e))
        })?;

        Ok(api_client)
    }

    #[instrument(skip_all)]
    async fn get_jwt_token(&mut self) -> Result<(), ScouterError> {
        let url = format!("{}/{}", self.base_path, Routes::AuthLogin.as_str());
        debug!("Getting JWT token from {}", url);

        let response = self.client.get(url).send().await.map_err(|e| {
            ScouterError::Error(format!("Failed to send request with error: {}", e))
        })?;

        // check if unauthorized
        if response.status().is_client_error() {
            error!("Failed to get JWT token: Unauthorized");
            return Err(ScouterError::Error("Unauthorized".to_string()));
        }

        let response = response.json::<JwtToken>().await.map_err(|e| {
            ScouterError::Error(format!("Failed to parse jwt token with error: {}", e))
        })?;

        self.config.auth_token = response.token;

        Ok(())
    }

    async fn update_token_from_response(&mut self, response: &Response) {
        if let Some(new_token) = response
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
        {
            self.config.auth_token = new_token.to_string();
        }
    }

    async fn _request(
        &self,
        route: Routes,
        request_type: RequestType,
        body_params: Option<Value>,
        query_string: Option<String>,
        headers: Option<HeaderMap>,
    ) -> Result<Response, ScouterError> {
        let headers = headers.unwrap_or_default();

        let url = format!("{}/{}", self.base_path, route.as_str());
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
                .bearer_auth(&self.config.auth_token)
                .send()
                .await
                .map_err(|e| {
                    ScouterError::Error(format!("Failed to send request with error: {}", e))
                })?,
            RequestType::Put => self
                .client
                .put(url)
                .headers(headers)
                .json(&body_params)
                .bearer_auth(&self.config.auth_token)
                .send()
                .await
                .map_err(|e| {
                    ScouterError::Error(format!("Failed to send request with error: {}", e))
                })?,
            RequestType::Delete => {
                let url = if let Some(query_string) = query_string {
                    format!("{}?{}", url, query_string)
                } else {
                    url
                };
                self.client
                    .delete(url)
                    .headers(headers)
                    .bearer_auth(&self.config.auth_token)
                    .send()
                    .await
                    .map_err(|e| {
                        ScouterError::Error(format!("Failed to send request with error: {}", e))
                    })?
            }
        };

        Ok(response)
    }

    pub async fn request(
        &mut self,
        route: Routes,
        request_type: RequestType,
        body_params: Option<Value>,
        query_params: Option<String>,
        headers: Option<HeaderMap>,
    ) -> Result<Response, ScouterError> {
        let response = self
            ._request(
                route.clone(),
                request_type,
                body_params,
                query_params,
                headers,
            )
            .await?;

        // Check and update token if a new one was provided
        self.update_token_from_response(&response).await;

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
            .request(
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

    pub async fn flush(&self) -> Result<(), ScouterError> {
        Ok(())
    }
}
