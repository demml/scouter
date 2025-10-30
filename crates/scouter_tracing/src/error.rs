use pyo3::exceptions::PyRuntimeError;
use pyo3::PyErr;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TraceError {
    #[error("{0}")]
    PyError(String),

    #[error("Failed to initialize tracer: {0}")]
    InitializationError(String),

    #[error("Span operation failed: {0}")]
    SpanError(String),

    #[error("OpenTelemetry error: {0}")]
    OpenTelemetryError(#[from] opentelemetry_otlp::ExporterBuildError),
}

impl From<TraceError> for PyErr {
    fn from(err: TraceError) -> PyErr {
        let msg = err.to_string();
        PyRuntimeError::new_err(msg)
    }
}

impl From<PyErr> for TraceError {
    fn from(err: PyErr) -> TraceError {
        TraceError::PyError(err.to_string())
    }
}
