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

    #[error(transparent)]
    PyErr(#[from] pyo3::PyErr),

    #[error("{0}")]
    DowncastError(String),

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
    ClusteringError(#[from] linfa_clustering::DbscanParamsError),

    #[error(transparent)]
    ReductionError(#[from] linfa_reduction::ReductionError),

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

impl<'a> From<pyo3::DowncastError<'a, 'a>> for EvaluationError {
    fn from(err: pyo3::DowncastError) -> Self {
        EvaluationError::DowncastError(err.to_string())
    }
}
