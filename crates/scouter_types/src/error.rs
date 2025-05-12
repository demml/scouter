use pyo3::exceptions::PyRuntimeError;
use pyo3::PyErr;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum UtilError {
    #[error("Failed to get parent path")]
    GetParentPathError,

    #[error("Failed to create directory")]
    CreateDirectoryError,

    #[error("Failed to read to create path")]
    CreatePathError,

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),
}

#[derive(Error, Debug)]
pub enum PyUtilError {
    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),

    #[error(transparent)]
    UtilError(#[from] UtilError),

    #[error(transparent)]
    PyErr(#[from] pyo3::PyErr),
}

impl From<PyUtilError> for PyErr {
    fn from(err: PyUtilError) -> PyErr {
        let msg = err.to_string();
        PyRuntimeError::new_err(msg)
    }
}

#[derive(Error, Debug)]
pub enum TypeError {
    #[error("Start time must be before end time")]
    StartTimeError,

    #[error("Invalid schedule")]
    InvalidScheduleError,

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

    #[error("Missing value for string feature")]
    MissingStringValueError,
}

#[derive(Error, Debug)]
pub enum PyTypeError {
    #[error(transparent)]
    PyErr(#[from] pyo3::PyErr),

    #[error("Failed to downcast Python object: {0}")]
    DowncastError(String),

    #[error(transparent)]
    TypeError(#[from] TypeError),
}

impl<'a> From<pyo3::DowncastError<'a, 'a>> for PyTypeError {
    fn from(err: pyo3::DowncastError) -> Self {
        PyTypeError::DowncastError(err.to_string())
    }
}

impl From<PyTypeError> for PyErr {
    fn from(err: PyTypeError) -> PyErr {
        let msg = err.to_string();
        PyRuntimeError::new_err(msg)
    }
}

#[derive(Error, Debug)]
pub enum ContractError {
    #[error(transparent)]
    PyErr(#[from] pyo3::PyErr),

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
    #[error("Unable to extract record into any known ServerRecord variant")]
    ExtractionError,

    #[error("No server records found")]
    EmptyServerRecordsError,

    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),

    #[error("Unexpected record type")]
    InvalidDriftTypeError,
}

#[derive(Error, Debug)]
pub enum PyRecordError {
    #[error(transparent)]
    RecordError(#[from] RecordError),

    #[error(transparent)]
    PyErr(#[from] pyo3::PyErr),
}

impl From<PyRecordError> for PyErr {
    fn from(err: PyRecordError) -> PyErr {
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

    #[error("Unexpected record type")]
    InvalidDriftTypeError,

    #[error(transparent)]
    UtilError(#[from] UtilError),

    #[error(transparent)]
    TypeError(#[from] TypeError),

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error("Missing sample argument")]
    MissingSampleError,

    #[error("Missing sample size argument")]
    MissingSampleSizeError,

    #[error("Custom alert thresholds have not been set")]
    CustomThresholdNotSetError,

    #[error("Custom alert threshold not found")]
    CustomAlertThresholdNotFound,
}

#[derive(Error, Debug)]
pub enum PyProfileError {
    #[error(transparent)]
    PyErr(#[from] pyo3::PyErr),

    #[error(transparent)]
    ProfileError(#[from] ProfileError),

    #[error(transparent)]
    TypeError(#[from] TypeError),

    #[error(transparent)]
    PyTypeError(#[from] PyTypeError),

    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    UtilError(#[from] UtilError),
}
impl From<PyProfileError> for PyErr {
    fn from(err: PyProfileError) -> PyErr {
        let msg = err.to_string();
        PyRuntimeError::new_err(msg)
    }
}
