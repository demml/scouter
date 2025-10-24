use pyo3::exceptions::PyRuntimeError;
use pyo3::PyErr;
use thiserror::Error;
use tracing::error;

#[derive(Error, Debug)]
pub enum EvaluationError {
    #[error("Invalid response type. Expected Score")]
    InvalidResponseError,

    #[error(transparent)]
    WorkflowError(#[from] potato_head::WorkflowError),

    #[error("{0}")]
    DowncastError(String),

    #[error(transparent)]
    JoinError(#[from] tokio::task::JoinError),

    #[error(transparent)]
    RegexError(#[from] regex::Error),

    #[error("Missing key: {0}")]
    MissingKeyError(String),

    #[error("Invalid context type. Context must be a PyDict or a Pydantic BaseModel")]
    MustBeDictOrBaseModel,

    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error(transparent)]
    TypeError(#[from] scouter_types::error::TypeError),

    #[error("Invalid embedder type. Expected an instance of Embedder")]
    InvalidEmbedderType,

    #[error("No results found in evaluation results")]
    NoResultsFound,

    #[error(transparent)]
    ClusteringError(#[from] linfa_clustering::DbscanParamsError),

    #[error(transparent)]
    ReductionError(#[from] linfa_reduction::ReductionError),

    #[error(transparent)]
    ShapeError(#[from] ndarray::ShapeError),

    #[error(transparent)]
    DataProfileError(#[from] scouter_profile::error::DataProfileError),

    #[error(transparent)]
    SerdeError(#[from] serde_json::Error),

    #[error("Field '{0}' not found")]
    FieldNotFound(String),

    #[error("Index {0} not found")]
    IndexNotFound(usize),

    #[error("Invalid array index: {0}")]
    InvalidArrayIndex(String),

    #[error("Empty field path provided")]
    EmptyFieldPath,

    #[error("{0}")]
    PyError(String),

    #[error("Cannot compare non-numeric values")]
    CannotCompareNonNumericValues,

    #[error("Contains operation requires string or list")]
    InvalidContainsOperation,

    #[error("StartsWith operation requires strings")]
    InvalidStartsWithOperation,

    #[error("EndsWith operation requires strings")]
    InvalidEndsWithOperation,

    #[error("Regex match requires strings")]
    InvalidRegexOperation,

    #[error("Invalid number format")]
    InvalidNumberFormat,

    #[error("Cannot convert object to AssertionValue")]
    CannotConvertObjectToAssertionValue,

    #[error("Cannot get length of object")]
    CannotGetLengthOfObject,

    #[error("Expected value for length must be an integer")]
    ExpectedLengthMustBeInteger,

    #[error("Invalid assertion value type")]
    InvalidAssertionValueType,

    #[error("Invalid task type for evaluation")]
    InvalidTaskType,
}

impl From<EvaluationError> for PyErr {
    fn from(err: EvaluationError) -> PyErr {
        let msg = err.to_string();
        error!("{}", msg);
        PyRuntimeError::new_err(msg)
    }
}

impl<'a> From<pyo3::DowncastError<'a, 'a>> for EvaluationError {
    fn from(err: pyo3::DowncastError) -> Self {
        EvaluationError::DowncastError(err.to_string())
    }
}

impl From<PyErr> for EvaluationError {
    fn from(err: PyErr) -> EvaluationError {
        EvaluationError::PyError(err.to_string())
    }
}
