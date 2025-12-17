use pyo3::prelude::*;
use scouter_types::PyHelperFuncs;
use scouter_types::TransportType;
use serde::Serialize;

#[pyclass]
#[derive(Debug, Clone, Serialize)]
pub struct GrpcConfig {
    #[pyo3(get, set)]
    pub server_uri: String,

    #[pyo3(get, set)]
    pub username: String,

    #[pyo3(get, set)]
    pub password: String,

    #[pyo3(get, set)]
    pub auth_token: String,

    #[pyo3(get)]
    pub transport_type: TransportType,
}

#[pymethods]
impl GrpcConfig {
    #[new]
    #[pyo3(signature = (server_uri=None, username=None, password=None, auth_token=None))]
    pub fn new(
        server_uri: Option<String>,
        username: Option<String>,
        password: Option<String>,
        auth_token: Option<String>,
    ) -> Self {
        // Use dedicated SCOUTER_GRPC_URI env var with sensible fallback
        let server_uri = server_uri.unwrap_or_else(|| {
            std::env::var("SCOUTER_GRPC_URI")
                .unwrap_or_else(|_| "http://localhost:50051".to_string())
        });

        let username = username.unwrap_or_else(|| {
            std::env::var("SCOUTER_USERNAME").unwrap_or_else(|_| "guest".to_string())
        });

        let password = password.unwrap_or_else(|| {
            std::env::var("SCOUTER_PASSWORD").unwrap_or_else(|_| "guest".to_string())
        });

        let auth_token = auth_token.unwrap_or_else(|| {
            std::env::var("SCOUTER_AUTH_TOKEN").unwrap_or_else(|_| "".to_string())
        });

        GrpcConfig {
            server_uri,
            username,
            password,
            auth_token,
            transport_type: TransportType::Grpc,
        }
    }

    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

impl Default for GrpcConfig {
    fn default() -> Self {
        GrpcConfig {
            server_uri: std::env::var("SCOUTER_GRPC_URI")
                .unwrap_or_else(|_| "http://localhost:50051".to_string()),
            username: std::env::var("SCOUTER_USERNAME").unwrap_or_else(|_| "guest".to_string()),
            password: std::env::var("SCOUTER_PASSWORD").unwrap_or_else(|_| "guest".to_string()),
            auth_token: std::env::var("SCOUTER_AUTH_TOKEN").unwrap_or_else(|_| "".to_string()),
            transport_type: TransportType::Grpc,
        }
    }
}
