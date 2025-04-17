// we redifine HTTPClient here because the scouterClient needs a blocking httpclient, unlike the producer enum

use reqwest::blocking::{Client, Response};
use reqwest::header;
use reqwest::header::{HeaderMap, HeaderValue};
use scouter_error::ScouterError;
use scouter_settings::http::HTTPConfig;
use scouter_types::http::{JwtToken, RequestType, Routes};
use serde_json::Value;
use tracing::{debug, error, instrument};

const TIMEOUT_SECS: u64 = 60;

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
    pub fn new(config: HTTPConfig) -> Result<Self, ScouterError> {
        let client = build_http_client(&config)?;

        let mut api_client = HTTPClient {
            client,
            config: config.clone(),
            base_path: format!("{}/{}", config.server_uri, "scouter"),
        };

        api_client.get_jwt_token().map_err(|e| {
            error!("Failed to get JWT token: {}", e);
            ScouterError::Error(format!("Failed to get JWT token with error: {}", e))
        })?;

        Ok(api_client)
    }

    #[instrument(skip_all)]
    fn get_jwt_token(&mut self) -> Result<(), ScouterError> {
        let url = format!("{}/{}", self.base_path, Routes::AuthLogin.as_str());
        debug!("Getting JWT token from {}", url);

        let response = self
            .client
            .get(url)
            .send()
            .map_err(ScouterError::traced_request_error)?;

        // check if unauthorized
        if response.status().is_client_error() {
            return Err(ScouterError::traced_unauthorized_error());
        }

        let response = response
            .json::<JwtToken>()
            .map_err(ScouterError::traced_parse_jwt_error)?;

        self.config.auth_token = response.token;

        Ok(())
    }

    fn update_token_from_response(&mut self, response: &Response) {
        if let Some(new_token) = response
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
        {
            self.config.auth_token = new_token.to_string();
        }
    }

    fn _request(
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
                    .map_err(ScouterError::traced_request_error)?
            }
            RequestType::Post => self
                .client
                .post(url)
                .headers(headers)
                .json(&body_params)
                .bearer_auth(&self.config.auth_token)
                .send()
                .map_err(ScouterError::traced_request_error)?,
            RequestType::Put => self
                .client
                .put(url)
                .headers(headers)
                .json(&body_params)
                .bearer_auth(&self.config.auth_token)
                .send()
                .map_err(ScouterError::traced_request_error)?,
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
                    .map_err(ScouterError::traced_request_error)?
            }
        };

        Ok(response)
    }

    pub fn request(
        &mut self,
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
