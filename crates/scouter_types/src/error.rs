use pyo3::exceptions::PyRuntimeError;
use pyo3::PyErr;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum UtilError {
    #[error("Failed to parse cron expression: {0}")]
    ParseCronError(String),

    #[error("Failed to serialize: {0}")]
    SerializeError(String),

    #[error("Failed to deserialize: {0}")]
    DeSerializeError(String),

    #[error("Failed to decode base64-encoded string: {0}")]
    DecodeBase64Error(String),

    #[error("Failed to convert string to Utf-8: {0}")]
    ConvertUtf8Error(String),

    #[error("Failed to set log level: {0}")]
    SetLogLevelError(String),

    #[error("Failed to get parent path")]
    GetParentPathError,

    #[error("Failed to create directory")]
    CreateDirectoryError,

    #[error("Failed to write to file")]
    WriteError,

    #[error("Failed to read to file")]
    ReadError,
}

#[derive(Error, Debug)]
pub enum TypeError {
    #[error("Start time must be before end time")]
    StartTimeError,

    #[error("Invalid schedule")]
    InvalidScheduleError,

    #[error(transparent)]
    PyErr(#[from] pyo3::PyErr),
}

impl From<TypeError> for PyErr {
    fn from(err: TypeError) -> PyErr {
        let msg = err.to_string();
        PyRuntimeError::new_err(msg)
    }
}

#[derive(Error, Debug)]
pub enum ContractError {
    #[error(transparent)]
    TypeError(#[from] TypeError),
}

impl From<ContractError> for PyErr {
    fn from(err: ContractError) -> PyErr {
        let msg = err.to_string();
        PyRuntimeError::new_err(msg)
    }
}
