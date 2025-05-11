use pyo3::exceptions::PyRuntimeError;
use pyo3::PyErr;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum UtilError {
    #[error("Failed to parse cron expression: {0}")]
    ParseCronError(String),

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

    #[error("Failed to read to create path")]
    CreatePathError,

    #[error(transparent)]
    PyErr(#[from] pyo3::PyErr),

    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),

    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

#[derive(Error, Debug)]
pub enum TypeError {
    #[error("Start time must be before end time")]
    StartTimeError,

    #[error("Invalid schedule")]
    InvalidScheduleError,

    #[error(transparent)]
    PyErr(#[from] pyo3::PyErr),

    #[error("Missing space argument")]
    MissingSpaceError,

    #[error("Missing name argument")]
    MissingNameError,

    #[error("Missing version argument")]
    MissingVersionError,

    #[error("Missing alert_config argument")]
    MissingAlertConfigError,

    #[error("No metrics found")]
    NoMetricsError,

    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),

    #[error("Invalid number")]
    InvalidNumberError,

    #[error("Root must be an object")]
    RootMustBeObject,

    #[error("Unsupported type: {0}")]
    UnsupportedTypeError(String),

    #[error("Failed to downcast Python object: {0}")]
    DowncastError(String),

    #[error("Invalid data type")]
    InvalidDataType,
}

impl<'a> From<pyo3::DowncastError<'a, 'a>> for TypeError {
    fn from(err: pyo3::DowncastError) -> Self {
        TypeError::DowncastError(err.to_string())
    }
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

#[derive(Error, Debug)]
pub enum RecordError {
    #[error(transparent)]
    PyErr(#[from] pyo3::PyErr),

    #[error("Unable to extract record into any known ServerRecord variant")]
    ExtractionError,

    #[error("No server records found")]
    EmptyServerRecordsError,

    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),

    #[error("Unexpected record type")]
    InvalidDriftTypeError,
}

impl From<RecordError> for PyErr {
    fn from(err: RecordError) -> PyErr {
        let msg = err.to_string();
        PyRuntimeError::new_err(msg)
    }
}

#[derive(Error, Debug)]
pub enum ProfileError {
    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),

    #[error("Features and array are not the same length")]
    FeatureArrayLengthError,

    #[error(transparent)]
    PyErr(#[from] pyo3::PyErr),

    #[error("Unexpected record type")]
    InvalidDriftTypeError,

    #[error(transparent)]
    UtilError(#[from] UtilError),

    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

impl From<ProfileError> for PyErr {
    fn from(err: ProfileError) -> PyErr {
        let msg = err.to_string();
        PyRuntimeError::new_err(msg)
    }
}
