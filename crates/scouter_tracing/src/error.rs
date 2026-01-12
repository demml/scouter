use pyo3::exceptions::PyRuntimeError;
use pyo3::pyclass::PyClassGuardError;
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
    OTelBuilderError(#[from] opentelemetry_otlp::ExporterBuildError),

    #[error("No active span found")]
    NoActiveSpan,

    #[error("Poison error occurred")]
    PoisonError(String),

    #[error(transparent)]
    OTelSdkError(#[from] opentelemetry_sdk::error::OTelSdkError),

    #[error(transparent)]
    TypeError(#[from] scouter_types::error::TypeError),

    #[error("Event must be a dictionary or a Pydantic BaseModel")]
    EventMustBeDict,

    #[error("Failed to downcast Python object: {0}")]
    DowncastError(String),

    #[error(transparent)]
    SerdeError(#[from] serde_json::Error),

    #[error("Invalid function type: {0}")]
    InvalidFunctionType(String),

    #[error("Unsupported SpanExporter type")]
    UnsupportedSpanExporterType,

    #[error(transparent)]
    EventError(#[from] scouter_events::error::EventError),

    #[error(transparent)]
    ParseIntError(#[from] std::num::ParseIntError),

    #[error("Queue not initialized")]
    QueueNotInitialized,
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

impl<'a, 'py> From<PyClassGuardError<'a, 'py>> for TraceError {
    fn from(err: PyClassGuardError<'a, 'py>) -> Self {
        TraceError::PyError(err.to_string())
    }
}
