use pyo3::prelude::*;
use scouter_types::PyHelperFuncs;
use scouter_types::TransportType;
use serde::{Serialize, Serializer};

fn redact<S: Serializer>(_: &str, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str("***")
}

#[pyclass]
#[derive(Clone, Serialize)]
pub struct GrpcConfig {
    #[pyo3(get, set)]
    pub server_uri: String,

    #[pyo3(get, set)]
    pub username: String,

    #[pyo3(set)]
    #[serde(serialize_with = "redact")]
    pub password: String,

    #[pyo3(get, set)]
    pub timeout_secs: Option<u64>,

    #[pyo3(get, set)]
    pub connect_timeout_secs: Option<u64>,

    #[pyo3(get, set)]
    pub keep_alive_interval_secs: Option<u64>,

    #[pyo3(get, set)]
    pub keep_alive_timeout_secs: Option<u64>,

    #[pyo3(get, set)]
    pub keep_alive_while_idle: Option<bool>,

    #[pyo3(get)]
    pub transport_type: TransportType,
}

impl std::fmt::Debug for GrpcConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GrpcConfig")
            .field("server_uri", &self.server_uri)
            .field("username", &self.username)
            .field("password", &"***")
            .field("timeout_secs", &self.timeout_secs)
            .field("connect_timeout_secs", &self.connect_timeout_secs)
            .field("keep_alive_interval_secs", &self.keep_alive_interval_secs)
            .field("keep_alive_timeout_secs", &self.keep_alive_timeout_secs)
            .field("keep_alive_while_idle", &self.keep_alive_while_idle)
            .field("transport_type", &self.transport_type)
            .finish()
    }
}

#[pymethods]
impl GrpcConfig {
    #[new]
    #[pyo3(signature = (
        server_uri=None,
        username=None,
        password=None,
        timeout_secs=None,
        connect_timeout_secs=None,
        keep_alive_interval_secs=None,
        keep_alive_timeout_secs=None,
        keep_alive_while_idle=None,
    ))]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        server_uri: Option<String>,
        username: Option<String>,
        password: Option<String>,
        timeout_secs: Option<u64>,
        connect_timeout_secs: Option<u64>,
        keep_alive_interval_secs: Option<u64>,
        keep_alive_timeout_secs: Option<u64>,
        keep_alive_while_idle: Option<bool>,
    ) -> Self {
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

        GrpcConfig {
            server_uri,
            username,
            password,
            timeout_secs,
            connect_timeout_secs,
            keep_alive_interval_secs,
            keep_alive_timeout_secs,
            keep_alive_while_idle,
            transport_type: TransportType::Grpc,
        }
    }

    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

impl Default for GrpcConfig {
    fn default() -> Self {
        Self::new(None, None, None, None, None, None, None, None)
    }
}
