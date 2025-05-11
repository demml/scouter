// we redifine HTTPClient here because the scouterClient needs a blocking httpclient, unlike the producer enum

use reqwest::blocking::{Client, Response};
use reqwest::header::AUTHORIZATION;
use reqwest::header::{HeaderMap, HeaderValue};
use scouter_error::{ClientError, ScouterError};
use scouter_settings::http::HTTPConfig;
use scouter_types::http::{JwtToken, RequestType, Routes};
use serde_json::Value;
use std::sync::Arc;
use std::sync::RwLock;
use tracing::{debug, error, instrument};

const TIMEOUT_SECS: u64 = 60;

/// Create a new HTTP client that can be shared across different clients
pub fn build_http_client(settings: &HTTPConfig) -> Result<Client, ClientError> {
    let mut headers = HeaderMap::new();

    headers.insert(
        "Username",
        HeaderValue::from_str(&settings.username)
            .map_err(ClientError::traced_create_header_error)?,
    );

    headers.insert(
        "Password",
        HeaderValue::from_str(&settings.password)
            .map_err(ClientError::traced_create_header_error)?,
    );

    let client_builder = Client::builder().timeout(std::time::Duration::from_secs(TIMEOUT_SECS));
    let client = client_builder
        .default_headers(headers)
        .build()
        .map_err(ClientError::traced_create_client_error)?;
    Ok(client)
}

#[derive(Debug, Clone)]
pub struct HTTPClient {
    client: Client,
    base_path: String,
    auth_token: Arc<RwLock<String>>,
}

impl HTTPClient {
    pub fn new(config: HTTPConfig) -> Result<Self, ClientError> {
        let client = build_http_client(&config)?;

        let api_client = HTTPClient {
            client,
            auth_token: Arc::new(RwLock::new(String::new())),
            base_path: format!("{}/{}", config.server_uri, "scouter"),
        };

        api_client
            .refresh_token()
            .map_err(ClientError::traced_jwt_error)?;

        Ok(api_client)
    }

    #[instrument(skip_all)]
    fn refresh_token(&self) -> Result<(), ScouterError> {
        let url = format!("{}/{}", self.base_path, Routes::AuthLogin.as_str());
        debug!("Getting JWT token from {}", url);

        let response = self
            .client
            .get(url)
            .send()
            .map_err(ClientError::traced_request_error)?;

        // check if unauthorized
        if response.status().is_client_error() {
            return Err(ScouterError::ClientError(ClientError::Unauthorized));
        }

        let token = response
            .json::<JwtToken>()
            .map_err(ClientError::traced_parse_jwt_error)?;

        if let Ok(mut token_guard) = self.auth_token.write() {
            *token_guard = token.token;
        } else {
            error!("Failed to acquire write lock for token update");
            return Err(ScouterError::ClientError(ClientError::UpdateAuthError));
        }

        Ok(())
    }

    fn update_token_from_response(&self, response: &Response) {
        if let Some(new_token) = response
            .headers()
            .get(AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
        {
            match self.auth_token.write() {
                Ok(mut token_guard) => {
                    *token_guard = new_token.to_string();
                }
                Err(e) => {
                    error!("Failed to acquire write lock for jwt token update: {}", e);
                }
            }
        }
    }

    fn get_current_token(&self) -> String {
        match self.auth_token.read() {
            Ok(token_guard) => token_guard.clone(),
            Err(e) => {
                error!("Failed to acquire read lock for token: {}", e);
                "".to_string()
            }
        }
    }

    fn _request(
        &self,
        route: Routes,
        request_type: RequestType,
        body_params: Option<Value>,
        query_string: Option<String>,
        headers: Option<HeaderMap>,
    ) -> Result<Response, ClientError> {
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
                    .bearer_auth(self.get_current_token())
                    .send()
                    .map_err(ClientError::traced_request_error)?
            }
            RequestType::Post => self
                .client
                .post(url)
                .headers(headers)
                .json(&body_params)
                .bearer_auth(self.get_current_token())
                .send()
                .map_err(ClientError::traced_request_error)?,
            RequestType::Put => self
                .client
                .put(url)
                .headers(headers)
                .json(&body_params)
                .bearer_auth(self.get_current_token())
                .send()
                .map_err(ClientError::traced_request_error)?,
            RequestType::Delete => {
                let url = if let Some(query_string) = query_string {
                    format!("{}?{}", url, query_string)
                } else {
                    url
                };
                self.client
                    .delete(url)
                    .headers(headers)
                    .bearer_auth(self.get_current_token())
                    .send()
                    .map_err(ClientError::traced_request_error)?
            }
        };

        Ok(response)
    }

    pub fn request(
        &self,
        route: Routes,
        request_type: RequestType,
        body_params: Option<Value>,
        query_params: Option<String>,
        headers: Option<HeaderMap>,
    ) -> Result<Response, ScouterError> {
        let response = self._request(
            route.clone(),
            request_type,
            body_params,
            query_params,
            headers,
        )?;

        // Check and update token if a new one was provided
        self.update_token_from_response(&response);

        Ok(response)
    }
}
