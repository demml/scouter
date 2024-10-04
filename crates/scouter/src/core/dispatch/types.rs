use pyo3::prelude::*;
use serde::{Deserialize, Serialize};

#[pyclass]
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
