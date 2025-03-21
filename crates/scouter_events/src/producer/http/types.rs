use pyo3::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct JwtToken {
    pub token: String,
}

#[derive(Debug, Clone)]
pub enum RequestType {
    Get,
    Post,
    Put,
}

#[derive(Debug, Clone)]
pub enum Routes {
    AuthApiLogin,
    AuthApiRefresh,
    Drift,
    SpcDrift,
    PsiDrift,
    CustomDrift,
    Profile,
    ProfileStatus,
    Alerts,
    DownloadProfile,
}

impl Routes {
    pub fn as_str(&self) -> &str {
        match self {
            Routes::AuthApiLogin => "auth/api/login",
            Routes::AuthApiRefresh => "auth/api/refresh",
            Routes::Profile => "profile",
            Routes::Drift => "drift",
            Routes::SpcDrift => "drift/spc",
            Routes::PsiDrift => "drift/psi",
            Routes::CustomDrift => "drift/custom",
            Routes::ProfileStatus => "profile/status",
            Routes::DownloadProfile => "profile/download",
            Routes::Alerts => "alerts",
        }
    }
}

#[pyclass]
#[derive(Debug, Clone)]
pub struct HTTPConfig {
    #[pyo3(get, set)]
    pub server_url: String,

    #[pyo3(get, set)]
    pub use_auth: bool,

    #[pyo3(get, set)]
    pub username: String,

    #[pyo3(get, set)]
    pub password: String,

    #[pyo3(get, set)]
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
            std::env::var("SCOUTER_SERVER_URL")
                .unwrap_or_else(|_| "http://localhost:8000/scouter".to_string())
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

impl Default for HTTPConfig {
    fn default() -> Self {
        HTTPConfig {
            server_url: std::env::var("SCOUTER_SERVER_URL")
                .unwrap_or_else(|_| "http://localhost:8000/scouter".to_string()),
            use_auth: false,
            username: std::env::var("SCOUTER_USERNAME").unwrap_or_else(|_| "admin".to_string()),
            password: std::env::var("SCOUTER_PASSWORD").unwrap_or_else(|_| "admin".to_string()),
            auth_token: std::env::var("SCOUTER_AUTH_TOKEN").unwrap_or_else(|_| "".to_string()),
        }
    }
}
