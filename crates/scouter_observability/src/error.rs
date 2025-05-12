use pyo3::PyErr;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ObservabilityError {
    #[error("Route not found {0}")]
    RouteNotFound(String),

    #[error("Failed to update route metrics: {0}")]
    UpdateMetricsError(String),

    #[error("Failed to compute quantiles: {0}")]
    QuantileError(String),

    #[error("Failed to collect metrics: {0}")]
    CollectMetricsError(String),
}

impl From<ObservabilityError> for PyErr {
    fn from(err: ObservabilityError) -> PyErr {
        pyo3::exceptions::PyRuntimeError::new_err(err.to_string())
    }
}
