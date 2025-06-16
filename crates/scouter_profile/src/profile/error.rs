use ndarray::Axis;
use pyo3::exceptions::PyRuntimeError;
use pyo3::PyErr;
use scouter_types::error::ProfileError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum QuantileError {
    #[error("Failed to compute quantile {quantile} for axis {axis:?}")]
    ComputeError {
        quantile: f64,
        axis: Axis,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

#[derive(Error, Debug)]
pub enum DataProfileError {
    #[error("Failed to parse JSON")]
    JsonParseError(#[from] serde_json::Error),

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

    #[error(transparent)]
    TypeError(#[from] scouter_types::error::TypeError),

    #[error("{0}")]
    PyError(String),

    #[error("{0}")]
    RuntimeError(String),

    #[error("Failed to compute quantile {0}")]
    Quantile(QuantileError),

    #[error("Failed to compute quantile error")]
    ComputeQuantileError,

    #[error(transparent)]
    QuantileError(#[from] ndarray_stats::errors::QuantileError),
}

impl From<DataProfileError> for PyErr {
    fn from(err: DataProfileError) -> PyErr {
        let msg = err.to_string();
        PyRuntimeError::new_err(msg)
    }
}

impl From<PyErr> for DataProfileError {
    fn from(err: PyErr) -> DataProfileError {
        DataProfileError::PyError(err.to_string())
    }
}
