use pyo3::exceptions::PyRuntimeError;
use pyo3::pyclass::PyClassGuardError;
use pyo3::PyErr;
use pythonize::PythonizeError;
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

    #[error("Cannot get length: {0}")]
    CannotGetLength(String),

    #[error(transparent)]
    ProfileError(#[from] scouter_types::error::ProfileError),

    #[error("Failed to process GenAI drift record: {0}")]
    GenAIEvaluatorError(String),

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Failed to acquire read lock on workflow")]
    ReadLockAcquireError,

    #[error("Invalid email validation operation")]
    InvalidEmailOperation,

    #[error("Invalid URL validation operation")]
    InvalidUrlOperation,

    #[error("Invalid UUID validation operation")]
    InvalidUuidOperation,

    #[error("Invalid ISO8601 validation operation")]
    InvalidIso8601Operation,

    #[error("Invalid JSON validation operation")]
    InvalidJsonOperation,

    #[error("Invalid range format - expected [min, max] array")]
    InvalidRangeFormat,

    #[error("Invalid tolerance format - expected [value, tolerance] array")]
    InvalidToleranceFormat,

    #[error("Invalid ContainsAll operation")]
    InvalidContainsAllOperation,

    #[error("Invalid ContainsAny operation")]
    InvalidContainsAnyOperation,

    #[error("Invalid ContainsNone operation")]
    InvalidContainsNoneOperation,

    #[error("Invalid empty check operation")]
    InvalidEmptyOperation,

    #[error("Invalid unique items check operation")]
    InvalidUniqueItemsOperation,

    #[error("Invalid alphabetic check operation")]
    InvalidAlphabeticOperation,

    #[error("Invalid alphanumeric check operation")]
    InvalidAlphanumericOperation,

    #[error("Invalid case check operation")]
    InvalidCaseOperation,

    #[error("Invalid contains word operation")]
    InvalidContainsWordOperation,

    #[error(transparent)]
    RecordError(#[from] scouter_types::error::RecordError),

    #[error("Array {index} out of bounds for length {length}")]
    IndexOutOfBounds { index: isize, length: usize },

    #[error("Expected an integer index or a slice")]
    IndexOrSliceExpected,

    #[error("Invalid sequence matches operation")]
    InvalidSequenceMatchesOperation,

    #[error("Invalid filter: {0}")]
    InvalidFilter(String),

    #[error("Trace data has no spans")]
    NoRootSpan,

    #[error("Attribute '{0}' not found in span")]
    AttributeNotFound(String),
}

impl From<pythonize::PythonizeError> for EvaluationError {
    fn from(err: PythonizeError) -> Self {
        EvaluationError::PyError(err.to_string())
    }
}

impl<'a, 'py> From<pyo3::CastError<'a, 'py>> for EvaluationError {
    fn from(err: pyo3::CastError) -> Self {
        EvaluationError::DowncastError(err.to_string())
    }
}

impl From<EvaluationError> for PyErr {
    fn from(err: EvaluationError) -> PyErr {
        let msg = err.to_string();
        error!("{}", msg);
        PyRuntimeError::new_err(msg)
    }
}

impl From<PyErr> for EvaluationError {
    fn from(err: PyErr) -> EvaluationError {
        EvaluationError::PyError(err.to_string())
    }
}

impl<'a, 'py> From<PyClassGuardError<'a, 'py>> for EvaluationError {
    fn from(err: PyClassGuardError<'a, 'py>) -> Self {
        EvaluationError::PyError(err.to_string())
    }
}
