use pyo3::exceptions::PyRuntimeError;
use pyo3::PyErr;
use scouter_types::error::ProfileError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DataProfileError {
    #[error("Failed to parse JSON")]
    JsonParseError(#[from] serde_json::Error),

    #[error(transparent)]
    PyErr(#[from] pyo3::PyErr),

    #[error("Failed to calculate mean")]
    MeanError,

    #[error(transparent)]
    MinMaxError(#[from] ndarray_stats::errors::MinMaxError),

    #[error("Failed to get max bin")]
    MaxBinError,

    #[error(transparent)]
    ProfileError(#[from] ProfileError),

    #[error(transparent)]
    ShapeError(#[from] ndarray::ShapeError),
}

impl From<DataProfileError> for PyErr {
    fn from(err: DataProfileError) -> PyErr {
        let msg = err.to_string();
        PyRuntimeError::new_err(msg)
    }
}
