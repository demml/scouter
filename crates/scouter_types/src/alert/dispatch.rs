use crate::drift::DriftArgs;
use crate::error::TypeError;
use pyo3::{prelude::*, IntoPyObjectExt};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::str::FromStr;
#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct ConsoleDispatchConfig {
    #[pyo3(get, set)]
    pub enabled: bool,
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct SlackDispatchConfig {
    #[pyo3(get, set)]
    pub channel: String,
}

#[pymethods]
impl SlackDispatchConfig {
    #[new]
    pub fn new(channel: String) -> PyResult<Self> {
        Ok(SlackDispatchConfig { channel })
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct OpsGenieDispatchConfig {
    #[pyo3(get, set)]
    pub team: String,

    #[pyo3(get, set)]
    pub priority: String,
}

#[pymethods]
impl OpsGenieDispatchConfig {
    #[new]
    #[pyo3(signature = (team, priority="P5"))]
    pub fn new(team: &str, priority: &str) -> PyResult<Self> {
        Ok(OpsGenieDispatchConfig {
            team: team.to_string(),
            priority: priority.to_string(),
        })
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum AlertDispatchConfig {
    Slack(SlackDispatchConfig),
    OpsGenie(OpsGenieDispatchConfig),
    Console(ConsoleDispatchConfig),
}

impl AlertDispatchConfig {
    pub fn dispatch_type(&self) -> AlertDispatchType {
        match self {
            AlertDispatchConfig::Slack(_) => AlertDispatchType::Slack,
            AlertDispatchConfig::OpsGenie(_) => AlertDispatchType::OpsGenie,
            AlertDispatchConfig::Console(_) => AlertDispatchType::Console,
        }
    }

    pub fn config<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        match self {
            AlertDispatchConfig::Slack(config) => config.clone().into_bound_py_any(py),
            AlertDispatchConfig::OpsGenie(config) => config.clone().into_bound_py_any(py),
            AlertDispatchConfig::Console(config) => config.clone().into_bound_py_any(py),
        }
    }
}

impl Default for AlertDispatchConfig {
    fn default() -> Self {
        AlertDispatchConfig::Console(ConsoleDispatchConfig { enabled: true })
    }
}

#[pyclass(eq)]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone, Default)]
pub enum AlertDispatchType {
    Slack,
    #[default]
    Console,
    OpsGenie,
}

#[pymethods]
impl AlertDispatchType {
    pub fn to_string(&self) -> &str {
        match self {
            AlertDispatchType::Slack => "Slack",
            AlertDispatchType::Console => "Console",
            AlertDispatchType::OpsGenie => "OpsGenie",
        }
    }
}

impl Display for AlertDispatchType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertDispatchType::Slack => write!(f, "Slack"),
            AlertDispatchType::Console => write!(f, "Console"),
            AlertDispatchType::OpsGenie => write!(f, "OpsGenie"),
        }
    }
}

pub trait DispatchAlertDescription {
    fn create_alert_description(&self, dispatch_type: AlertDispatchType) -> String;
}

pub trait DispatchDriftConfig {
    fn get_drift_args(&self) -> DriftArgs;
}

#[pyclass]
#[derive(Debug, PartialEq, Clone, Serialize)]
pub enum TransportType {
    Kafka,
    RabbitMQ,
    Http,
    Redis,
    Mock,
}

#[pyclass(eq)]
#[derive(PartialEq, Clone, Debug, Serialize)]
pub enum CompressionType {
    None,
    Gzip,
    Snappy,
    Lz4,
    Zstd,
}

impl FromStr for CompressionType {
    type Err = TypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "none" => Ok(CompressionType::None),
            "gzip" => Ok(CompressionType::Gzip),
            "snappy" => Ok(CompressionType::Snappy),
            "lz4" => Ok(CompressionType::Lz4),
            "zstd" => Ok(CompressionType::Zstd),
            _ => Err(TypeError::InvalidCompressionTypeError),
        }
    }
}

// impl display
impl std::fmt::Display for CompressionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompressionType::None => write!(f, "none"),
            CompressionType::Gzip => write!(f, "gzip"),
            CompressionType::Snappy => write!(f, "snappy"),
            CompressionType::Lz4 => write!(f, "lz4"),
            CompressionType::Zstd => write!(f, "zstd"),
        }
    }
}

impl CompressionType {
    pub fn to_otel_compression(&self) -> Result<opentelemetry_otlp::Compression, TypeError> {
        match self {
            CompressionType::Gzip => Ok(opentelemetry_otlp::Compression::Gzip),
            CompressionType::Zstd => Ok(opentelemetry_otlp::Compression::Zstd),
            _ => Err(TypeError::CompressionTypeNotSupported(self.to_string())),
        }
    }
}
