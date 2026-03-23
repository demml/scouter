use pyo3::exceptions::PyRuntimeError;
use pyo3::PyErr;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatasetClientError {
    #[error(transparent)]
    Dataset(#[from] scouter_types::dataset::DatasetError),

    /// Hand-constructed error messages (e.g., lock poisoned, unregistered table).
    #[error("gRPC error: {0}")]
    GrpcError(String),

    #[error(transparent)]
    GrpcClient(#[from] scouter_tonic::error::ClientError),

    #[error(transparent)]
    Arrow(#[from] arrow::error::ArrowError),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("Client has been shut down")]
    AlreadyShutdown,

    #[error("Channel closed — producer may have been shut down")]
    ChannelClosed,

    #[error(transparent)]
    Event(#[from] scouter_events::error::EventError),

    #[error("{0}")]
    PyError(String),

    #[error("Fingerprint mismatch for table: expected {expected}, got {actual}")]
    FingerprintMismatch { expected: String, actual: String },
}

impl From<DatasetClientError> for PyErr {
    fn from(err: DatasetClientError) -> PyErr {
        PyRuntimeError::new_err(err.to_string())
    }
}

impl From<PyErr> for DatasetClientError {
    fn from(err: PyErr) -> Self {
        DatasetClientError::PyError(err.to_string())
    }
}

impl<T> From<tokio::sync::mpsc::error::SendError<T>> for DatasetClientError {
    fn from(_: tokio::sync::mpsc::error::SendError<T>) -> Self {
        DatasetClientError::ChannelClosed
    }
}
