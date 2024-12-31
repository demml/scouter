use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use crate::drift::DriftArgs;

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
    #[getter]
    pub fn value(&self) -> String {
        match self {
            AlertDispatchType::Slack => "Slack".to_string(),
            AlertDispatchType::Console => "Console".to_string(),
            AlertDispatchType::OpsGenie => "OpsGenie".to_string(),
        }
    }
}


pub trait DispatchAlertDescription {
    fn create_alert_description(&self, dispatch_type: AlertDispatchType) -> String;
}



pub trait DispatchDriftConfig {
    fn get_drift_args(&self) -> DriftArgs;
}