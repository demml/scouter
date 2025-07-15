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

    #[error("Invalid configuration provided for drifter. Please check that the configuration type matches the drifter type")]
    InvalidConfigError,

    #[error("Not implemented")]
    NotImplemented,

    #[error("Data type not supported: {0}")]
    UnsupportedDataTypeError(String),

    #[error("Failed to downcast Python object: {0}")]
    DowncastError(String),

    #[error(transparent)]
    ProfileError(#[from] scouter_types::error::ProfileError),

    #[error("Invalid drift type")]
    InvalidDriftType,

    #[error("Error processing alert: {0}")]
    AlertProcessingError(String),

    #[error("Feature to monitor: {0}, not present in data")]
    FeatureToMonitorMissingError(String),

    #[error("Categorical feature specified in drift config: {0}, not present in data")]
    CategoricalFeatureMissingError(String),

    #[error("Error computing quantiles: {0}")]
    QuantileComputationError(String),

    #[error("Insufficient Data Error: {0}")]
    InsufficientDataError(String),

    #[error("{0}")]
    InvalidParameterError(String),

    #[error("{0}")]
    InvalidBinCountError(String),

    #[error("{0}")]
    InvalidValueError(String),
}

impl<'a> From<pyo3::DowncastError<'a, 'a>> for DriftError {
    fn from(err: pyo3::DowncastError) -> Self {
        DriftError::DowncastError(err.to_string())
    }
}

impl From<DriftError> for PyErr {
    fn from(err: DriftError) -> PyErr {
        let msg = err.to_string();
        PyRuntimeError::new_err(msg)
    }
}

impl From<PyErr> for DriftError {
    fn from(err: PyErr) -> DriftError {
        DriftError::RunTimeError(err.to_string())
    }
}
