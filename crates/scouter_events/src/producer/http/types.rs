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
    Delete,
}

#[derive(Debug, Clone)]
pub enum Routes {
    AuthLogin,
    AuthRefresh,
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
            Routes::AuthLogin => "auth/login",
            Routes::AuthRefresh => "auth/refresh",
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
    pub server_uri: String,

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
    #[pyo3(signature = (server_uri=None, username=None, password=None, auth_token=None))]
    pub fn new(
        server_uri: Option<String>,
        username: Option<String>,
        password: Option<String>,
        auth_token: Option<String>,
    ) -> Self {
        let server_uri = server_uri.unwrap_or_else(|| {
            std::env::var("SCOUTER_SERVER_URI")
                .unwrap_or_else(|_| "http://localhost:8000".to_string())
        });
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
            server_uri,
            username,
            password,
            auth_token,
        }
    }
}

impl Default for HTTPConfig {
    fn default() -> Self {
        HTTPConfig {
            server_uri: std::env::var("SCOUTER_SERVER_URI")
                .unwrap_or_else(|_| "http://localhost:8000".to_string()),
            username: std::env::var("SCOUTER_USERNAME").unwrap_or_else(|_| "guest".to_string()),
            password: std::env::var("SCOUTER_PASSWORD").unwrap_or_else(|_| "guest".to_string()),
            auth_token: std::env::var("SCOUTER_AUTH_TOKEN").unwrap_or_else(|_| "".to_string()),
        }
    }
}
