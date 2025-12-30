use futures::io;
use potato_head::error::WorkflowError;
use pyo3::exceptions::PyRuntimeError;
use pyo3::pyclass::PyClassGuardError;
use pyo3::PyErr;
use scouter_dispatch::error::DispatchError;
#[cfg(feature = "sql")]
use scouter_sql::sql::error::SqlError;
use thiserror::Error;
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

    #[error(transparent)]
    UtilError(#[from] potato_head::UtilError),

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

    #[error("Empty Array Detected: {0}")]
    EmptyArrayError(String),

    #[error("Failed to compute binning edges: {0}")]
    BinningError(String),

    #[error("Failed to deserialize: {0}")]
    SerdeJsonError(#[from] serde_json::Error),

    #[error("Context is not a valid JSON object. Should be a Map<String, Value>")]
    InvalidContextFormat,

    #[error(transparent)]
    WorkflowError(#[from] WorkflowError),

    #[error("Incorrect method called: {0}")]
    WrongMethodError(String),

    #[error("Invalid content type. Expected a json string or value")]
    InvalidContentTypeError,

    #[error("Failed to setup tokio runtime for computing LLM drift: {0}")]
    SetupTokioRuntimeError(#[source] io::Error),

    #[error("Failed to process LLM drift record: {0}")]
    LLMEvaluatorError(String),

    #[error("{0}")]
    InvalidDataConfiguration(String),
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

impl<'a, 'py> From<PyClassGuardError<'a, 'py>> for DriftError {
    fn from(err: PyClassGuardError<'a, 'py>) -> Self {
        DriftError::RunTimeError(err.to_string())
    }
}
