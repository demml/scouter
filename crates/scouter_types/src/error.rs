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

impl From<UtilError> for PyErr {
    fn from(err: UtilError) -> PyErr {
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

    #[error("Invalid PSI threshold configuration")]
    InvalidPsiThresholdError,

    #[error("Invalid alert dispatch configuration")]
    InvalidDispatchConfigError,

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

    #[error("{0}")]
    PyError(String),

    #[error(
        "Unsupported feature type. Supported types are float, integer and string. Received: {0}"
    )]
    UnsupportedFeatureTypeError(String),

    #[error("Unsupported features type. Features must be a list of Feature instances or a dictionary of key value pairs. Received: {0}")]
    UnsupportedFeaturesTypeError(String),

    #[error("Unsupported metrics type. Metrics must be a list of Metric instances or a dictionary of key value pairs. Received: {0}")]
    UnsupportedMetricsTypeError(String),
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

impl From<PyErr> for TypeError {
    fn from(err: PyErr) -> TypeError {
        TypeError::PyError(err.to_string())
    }
}

#[derive(Error, Debug)]
pub enum ContractError {
    #[error(transparent)]
    TypeError(#[from] TypeError),

    #[error("{0}")]
    PyError(String),
}

impl From<ContractError> for PyErr {
    fn from(err: ContractError) -> PyErr {
        let msg = err.to_string();
        PyRuntimeError::new_err(msg)
    }
}

impl From<PyErr> for ContractError {
    fn from(err: PyErr) -> ContractError {
        ContractError::PyError(err.to_string())
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

    #[error("{0}")]
    PyError(String),
}

impl From<RecordError> for PyErr {
    fn from(err: RecordError) -> PyErr {
        let msg = err.to_string();
        PyRuntimeError::new_err(msg)
    }
}

impl From<PyErr> for RecordError {
    fn from(err: PyErr) -> RecordError {
        RecordError::PyError(err.to_string())
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

    #[error("{0}")]
    PyError(String),
}

impl From<ProfileError> for PyErr {
    fn from(err: ProfileError) -> PyErr {
        let msg = err.to_string();
        PyRuntimeError::new_err(msg)
    }
}

impl From<PyErr> for ProfileError {
    fn from(err: PyErr) -> ProfileError {
        ProfileError::PyError(err.to_string())
    }
}
