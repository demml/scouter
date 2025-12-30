use pyo3::exceptions::PyRuntimeError;
use pyo3::PyErr;
use thiserror::Error;

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

    #[error("Invalid config type. Expected HttpConfig")]
    InvalidConfigTypeError,

    #[error("Failed to get drift data")]
    GetDriftDataError,

    #[error("{0}")]
    PyError(String),

    #[error(transparent)]
    UtilError(#[from] scouter_types::error::UtilError),

    #[error("Failed to parse JWT token from response: {0}")]
    ParseJwtTokenError(String),

    #[error("Failed to get paginated traces")]
    GetPaginatedTracesError,

    #[error("Failed to refresh trace summary")]
    RefreshTraceSummaryError,

    #[error("Failed to get trace spans")]
    GetTraceSpansError,

    #[error("Failed to get trace metrics")]
    GetTraceMetricsError,

    #[error("Failed to get trace baggage")]
    GetTraceBaggageError,

    #[error("Failed to get tags")]
    GetTagsError,
}

impl From<ClientError> for PyErr {
    fn from(err: ClientError) -> PyErr {
        let msg = err.to_string();
        PyRuntimeError::new_err(msg)
    }
}

impl From<PyErr> for ClientError {
    fn from(err: PyErr) -> ClientError {
        ClientError::PyError(err.to_string())
    }
}
