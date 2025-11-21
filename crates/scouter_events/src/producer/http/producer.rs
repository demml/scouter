// ScouterProducer expects an async client

use crate::error::EventError;
use reqwest::header;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client, Response};
use scouter_settings::HttpConfig;
use scouter_types::{JwtToken, MessageRecord, RequestType, Routes};
use serde_json::Value;
use std::sync::Arc;
// Using tokio RwLock for async read/write access since this is run within a spawned task
use tokio::sync::RwLock;
use tracing::{debug, instrument};
const TIMEOUT_SECS: u64 = 60;

/// Create a new HTTP client that can be shared across different clients
pub fn build_http_client(settings: &HttpConfig) -> Result<Client, EventError> {
    let mut headers = HeaderMap::new();

    headers.insert("Username", HeaderValue::from_str(&settings.username)?);

    headers.insert("Password", HeaderValue::from_str(&settings.password)?);

    let client_builder = Client::builder().timeout(std::time::Duration::from_secs(TIMEOUT_SECS));
    let client = client_builder.default_headers(headers).build()?;
    Ok(client)
}

#[derive(Debug, Clone)]
pub struct HttpClient {
    client: Client,
    config: Arc<RwLock<HttpConfig>>,
    base_path: String,
}

impl HttpClient {
    pub async fn new(config: HttpConfig) -> Result<Self, EventError> {
        let client = build_http_client(&config)?;

        let mut api_client = HttpClient {
            client,
            config: Arc::new(RwLock::new(config.clone())),
            base_path: format!("{}/{}", config.server_uri, "scouter"),
        };

        api_client.get_jwt_token().await?;

        Ok(api_client)
    }

    #[instrument(skip_all)]
    async fn get_jwt_token(&mut self) -> Result<(), EventError> {
        let url = format!("{}/{}", self.base_path, Routes::AuthLogin.as_str());
        debug!("Getting JWT token from {}", url);

        let response = self.client.get(url).send().await?;

        // check if unauthorized
        if response.status().is_client_error() {
            return Err(EventError::UnauthorizedError);
        }

        let response = response.json::<JwtToken>().await?;

        self.config.write().await.auth_token = response.token;

        Ok(())
    }

    async fn update_token_from_response(&self, response: &Response) {
        if let Some(new_token) = response
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
        {
            self.config.write().await.auth_token = new_token.to_string();
        }
    }

    async fn _request(
        &self,
        route: Routes,
        request_type: RequestType,
        body_params: Option<Value>,
        query_string: Option<String>,
        headers: Option<HeaderMap>,
    ) -> Result<Response, EventError> {
        let headers = headers.unwrap_or_default();

        let url = format!("{}/{}", self.base_path, route.as_str());

        let config_lock = self.config.read().await;

        let response = match request_type {
            RequestType::Get => {
                let url = if let Some(query_string) = query_string {
                    format!("{url}?{query_string}")
                } else {
                    url
                };

                self.client
                    .get(url)
                    .headers(headers)
                    .bearer_auth(&config_lock.auth_token)
                    .send()
                    .await?
            }
            RequestType::Post => {
                self.client
                    .post(url)
                    .headers(headers)
                    .json(&body_params)
                    .bearer_auth(&config_lock.auth_token)
                    .send()
                    .await?
            }
            RequestType::Put => {
                self.client
                    .put(url)
                    .headers(headers)
                    .json(&body_params)
                    .bearer_auth(&config_lock.auth_token)
                    .send()
                    .await?
            }
            RequestType::Delete => {
                let url = if let Some(query_string) = query_string {
                    format!("{url}?{query_string}")
                } else {
                    url
                };
                self.client
                    .delete(url)
                    .headers(headers)
                    .bearer_auth(&config_lock.auth_token)
                    .send()
                    .await?
            }
        };

        Ok(response)
    }

    pub async fn request(
        &self,
        route: Routes,
        request_type: RequestType,
        body_params: Option<Value>,
        query_params: Option<String>,
        headers: Option<HeaderMap>,
    ) -> Result<Response, EventError> {
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
pub struct HttpProducer {
    client: HttpClient,
}

impl HttpProducer {
    pub async fn new(config: HttpConfig) -> Result<Self, EventError> {
        let client = HttpClient::new(config).await?;
        Ok(HttpProducer { client })
    }

    pub async fn publish(&self, message: MessageRecord) -> Result<(), EventError> {
        let serialized_msg: Value = serde_json::to_value(&message)?;

        let response = self
            .client
            .request(
                Routes::Message,
                RequestType::Post,
                Some(serialized_msg),
                None,
                None,
            )
            .await?;

        debug!("Published message to drift with response: {:?}", response);

        Ok(())
    }

    pub async fn flush(&self) -> Result<(), EventError> {
        Ok(())
    }
}
