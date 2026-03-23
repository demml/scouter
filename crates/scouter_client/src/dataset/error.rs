use pyo3::exceptions::PyRuntimeError;
use pyo3::PyErr;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatasetClientError {
    #[error(transparent)]
    DatasetError(#[from] scouter_types::dataset::DatasetError),

    #[error("gRPC error: {0}")]
    GrpcError(String),

    #[error("IPC error: {0}")]
    IpcError(String),

    #[error("Client has been shut down")]
    AlreadyShutdown,

    #[error("Channel closed — producer may have been shut down")]
    ChannelClosed,

    #[error("Event error: {0}")]
    EventError(String),

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

impl From<scouter_tonic::error::ClientError> for DatasetClientError {
    fn from(err: scouter_tonic::error::ClientError) -> Self {
        DatasetClientError::GrpcError(err.to_string())
    }
}

impl From<arrow::error::ArrowError> for DatasetClientError {
    fn from(err: arrow::error::ArrowError) -> Self {
        DatasetClientError::IpcError(err.to_string())
    }
}

impl From<scouter_events::error::EventError> for DatasetClientError {
    fn from(err: scouter_events::error::EventError) -> Self {
        DatasetClientError::EventError(err.to_string())
    }
}

impl<T> From<tokio::sync::mpsc::error::SendError<T>> for DatasetClientError {
    fn from(_: tokio::sync::mpsc::error::SendError<T>) -> Self {
        DatasetClientError::ChannelClosed
    }
}
