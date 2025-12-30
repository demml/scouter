use pyo3::exceptions::PyRuntimeError;
use pyo3::pyclass::PyClassGuardError;
use pyo3::PyErr;
use thiserror::Error;
use tracing::error;
#[derive(Error, Debug)]
pub enum EvaluationError {
    #[error("Invalid response type. Expected Score")]
    InvalidResponseError,

    #[error(transparent)]
    WorkflowError(#[from] potato_head::WorkflowError),

    #[error(transparent)]
    PyErr(#[from] pyo3::PyErr),

    #[error("{0}")]
    Error(String),

    #[error(transparent)]
    JoinError(#[from] tokio::task::JoinError),

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
}

impl From<EvaluationError> for PyErr {
    fn from(err: EvaluationError) -> PyErr {
        let msg = err.to_string();
        error!("{}", msg);
        PyRuntimeError::new_err(msg)
    }
}

impl<'a, 'py> From<PyClassGuardError<'a, 'py>> for EvaluationError {
    fn from(err: PyClassGuardError<'a, 'py>) -> Self {
        EvaluationError::Error(err.to_string())
    }
}
