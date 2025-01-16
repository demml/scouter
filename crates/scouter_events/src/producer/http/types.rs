use serde::{Deserialize, Serialize};
use pyo3::prelude::*;

#[derive(Serialize, Deserialize)]
pub struct JwtToken {
    pub token: String,
}


#[derive(Debug, Clone)]
pub enum RequestType {
    Get,
    Post,
}

#[derive(Debug, Clone)]
pub enum Routes {
    AuthApiLogin,
    AuthApiRefresh,
    Drift,
}

impl Routes {
    pub fn as_str(&self) -> &str {
        match self {
            Routes::AuthApiLogin => "auth/api/login",
            Routes::AuthApiRefresh => "auth/api/refresh",
            Routes::Drift => "drift",
        }
    }
}

#[pyclass]
#[derive(Debug, Clone)]
pub struct HTTPConfig {
    pub server_url: String,
    pub use_auth: bool,
    pub username: String,
    pub password: String,
    pub auth_token: String,
}

#[pymethods]
impl HTTPConfig {
    #[new]
    #[pyo3(signature = (server_url=None, use_auth=false, username=None, password=None, auth_token=None))]
    pub fn new(
        server_url: Option<String>,
        use_auth: Option<bool>,
        username: Option<String>,
        password: Option<String>,
        auth_token: Option<String>,
    ) -> Self {
        let server_url = server_url.unwrap_or_else(|| {
            std::env::var("HTTP_SERVER_URL")
                .unwrap_or_else(|_| "http://localhost:8000".to_string())
        });
        let use_auth = use_auth.unwrap_or(false);
        let username = username.unwrap_or_else(|| {
            std::env::var("SCOUTER_USERNAME").unwrap_or_else(|_| "admin".to_string())
        });
        let password = password.unwrap_or_else(|| {
            std::env::var("SCOUTER_PASSWORD").unwrap_or_else(|_| "admin".to_string())
        });
        let auth_token = auth_token.unwrap_or_else(|| {
            std::env::var("SCOUTER_AUTH_TOKEN").unwrap_or_else(|_| "".to_string())
        });
   

        HTTPConfig {
            server_url,
            use_auth,
            username,
            password,
            auth_token,
        }
    }
}