use pyo3::exceptions::PyRuntimeError;
use pyo3::PyErr;
use scouter_dispatch::error::DispatchError;
use thiserror::Error;

#[cfg(feature = "sql")]
use scouter_sql::sql::error::SqlError;

#[derive(Error, Debug)]
pub enum DriftError {
    #[error("Failed to compute mean")]
    ComputeMeanError,

    #[error("At least 10 values needed to compute deciles")]
    NotEnoughDecileValuesError,

    #[error("Failed to convert deciles to array")]
    ConvertDecileToArray,

    #[error("Failed to compute deciles")]
    ComputeDecilesError,

    #[error("{0}")]
    RunTimeError(String),

    #[error("Feature and array length mismatch")]
    FeatureLengthError,

    #[error("Feature does not exist")]
    FeatureNotExistError,

    #[error(transparent)]
    ShapeError(#[from] ndarray::ShapeError),

    #[cfg(feature = "sql")]
    #[error(transparent)]
    SqlError(#[from] SqlError),

    #[error("SPC rule length is not 8")]
    SpcRuleLengthError,

    #[error(transparent)]
    ParseIntError(#[from] std::num::ParseIntError),

    #[error(transparent)]
    DispatchError(#[from] DispatchError),

    #[error("Failed to process alerts")]
    ProcessAlertError,
}

#[derive(Error, Debug)]
pub enum PyDriftError {
    #[error(transparent)]
    DriftError(#[from] DriftError),

    #[error(transparent)]
    PyErr(#[from] pyo3::PyErr),
}

impl From<PyDriftError> for PyErr {
    fn from(err: PyDriftError) -> PyErr {
        let msg = err.to_string();
        PyRuntimeError::new_err(msg)
    }
}
