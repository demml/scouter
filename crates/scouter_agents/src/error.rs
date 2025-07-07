use potato_head::AgentError;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MetricError {
    #[error("Missing required field: {0}")]
    MissingField(String),
    #[error("Validation error: {0}")]
    ValidationError(String),
    #[error("Execution error: {0}")]
    ExecutionError(String),
    #[error("Error: {0}")]
    Error(String),
    #[error(transparent)]
    AgentError(#[from] AgentError),
    #[error(transparent)]
    SerdeError(#[from] serde_json::Error),
}

impl From<MetricError> for PyErr {
    fn from(err: MetricError) -> PyErr {
        let msg = err.to_string();
        PyRuntimeError::new_err(msg)
    }
}

impl From<PyErr> for MetricError {
    fn from(err: PyErr) -> MetricError {
        MetricError::Error(err.to_string())
    }
}
