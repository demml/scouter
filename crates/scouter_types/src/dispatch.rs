use crate::drift::DriftArgs;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};

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
}

#[pymethods]
impl OpsGenieDispatchConfig {
    #[new]
    pub fn new(team: String) -> PyResult<Self> {
        Ok(OpsGenieDispatchConfig { team })
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum AlertDispatchConfig {
    Slack(SlackDispatchConfig),
    OpsGenie(OpsGenieDispatchConfig),
    Console(ConsoleDispatchConfig),
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

pub trait DispatchAlertDescription {
    fn create_alert_description(&self, dispatch_type: AlertDispatchType) -> String;
}

pub trait DispatchDriftConfig {
    fn get_drift_args(&self) -> DriftArgs;
}
