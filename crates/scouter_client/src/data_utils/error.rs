use pyo3::exceptions::PyRuntimeError;
use pyo3::PyErr;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PyDataError {
    #[error(transparent)]
    PyErr(#[from] pyo3::PyErr),

    #[error("Failed to downcast Python object: {0}")]
    DowncastError(String),

    #[error("Data type not supported: {0}")]
    UnsupportedDataTypeError(String),

    #[error("Data type must be a numpy array")]
    NotNumpyArrayError,

    #[error("Column names must be strings")]
    ColumnNamesMustBeStrings,
}
impl<'a> From<pyo3::DowncastError<'a, 'a>> for PyDataError {
    fn from(err: pyo3::DowncastError) -> Self {
        PyDataError::DowncastError(err.to_string())
    }
}

impl From<PyDataError> for PyErr {
    fn from(err: PyDataError) -> PyErr {
        let msg = err.to_string();
        PyRuntimeError::new_err(msg)
    }
}
