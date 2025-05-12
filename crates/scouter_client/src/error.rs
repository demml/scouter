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

#[derive(Error, Debug)]
pub enum ClientError {
    #[error(transparent)]
    HeaderError(#[from] reqwest::header::InvalidHeaderValue),

    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),

    #[error("Unauthorized")]
    Unauthorized,

    #[error("Failed to update auth token")]
    UpdateAuthTokenError,

    #[error("Failed to insert profile")]
    InsertProfileError,

    #[error("Failed to update profile")]
    UpdateProfileError,

    #[error("Failed to get drift alerts")]
    GetDriftAlertError,

    #[error("Failed to get drift profile")]
    GetDriftProfileError,

    #[error(transparent)]
    SerdeQsError(#[from] serde_qs::Error),

    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),
}

#[derive(Error, Debug)]
pub enum PyClientError {
    #[error(transparent)]
    ClientError(#[from] ClientError),

    #[error(transparent)]
    PyErr(#[from] pyo3::PyErr),

    #[error("Invalid config type. Expected HTTPConfig")]
    InvalidConfigTypeError,

    #[error("Failed to get drift data")]
    GetDriftDataError,

    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),

    #[error(transparent)]
    SerdeQsError(#[from] serde_qs::Error),

    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),

    #[error(transparent)]
    UtilError(#[from] scouter_types::error::UtilError),
}

impl From<PyClientError> for PyErr {
    fn from(err: PyClientError) -> PyErr {
        let msg = err.to_string();
        PyRuntimeError::new_err(msg)
    }
}
