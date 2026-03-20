use pyo3::exceptions::PyRuntimeError;
use pyo3::pyclass::PyClassGuardError;
use pyo3::PyErr;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatasetError {
    #[error("Schema parse error: {0}")]
    SchemaParseError(String),

    #[error("Unsupported JSON Schema type: {0}")]
    UnsupportedType(String),

    #[error("Failed to resolve $ref: {0}")]
    RefResolutionError(String),

    #[error("Schema fingerprint mismatch — expected {expected}, got {actual}")]
    FingerprintMismatch { expected: String, actual: String },

    #[error(transparent)]
    SerializationError(#[from] serde_json::Error),

    #[error("Arrow schema serialization error: {0}")]
    ArrowSchemaError(String),

    #[error("{0}")]
    PyError(String),
}

impl From<DatasetError> for PyErr {
    fn from(err: DatasetError) -> PyErr {
        let msg = err.to_string();
        PyRuntimeError::new_err(msg)
    }
}

impl From<PyErr> for DatasetError {
    fn from(err: PyErr) -> DatasetError {
        DatasetError::PyError(err.to_string())
    }
}

impl<'a, 'py> From<PyClassGuardError<'a, 'py>> for DatasetError {
    fn from(err: PyClassGuardError<'a, 'py>) -> Self {
        DatasetError::PyError(err.to_string())
    }
}
