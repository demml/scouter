use pyo3::exceptions::PyRuntimeError;
use pyo3::PyErr;
use scouter_drift::error::DriftError;
use scouter_profile::DataProfileError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DataError {
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

    #[error("Detected missing, Nan, or infinite values in the data. Scouter does not currently support these value types")]
    MissingNanOrInfiniteValues,
}
impl<'a> From<pyo3::DowncastError<'a, 'a>> for DataError {
    fn from(err: pyo3::DowncastError) -> Self {
        DataError::DowncastError(err.to_string())
    }
}

impl From<DataError> for PyErr {
    fn from(err: DataError) -> PyErr {
        let msg = err.to_string();
        PyRuntimeError::new_err(msg)
    }
}

impl From<DataError> for DriftError {
    fn from(err: DataError) -> Self {
        DriftError::RunTimeError(err.to_string())
    }
}

impl From<DataError> for DataProfileError {
    fn from(err: DataError) -> Self {
        DataProfileError::RuntimeError(err.to_string())
    }
}
