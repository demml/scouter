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
        "Invalid prompt response type. Expect Score as the output type for the LLMDriftMetric prompt"
    )]
    InvalidResponseType,

    #[error(
        "Unsupported feature type. Feature must be an integer, float or string. Received: {0}"
    )]
    UnsupportedFeatureTypeError(String),

    #[error("Unsupported features type. Features must be a list of Feature instances or a dictionary of key value pairs. Received: {0}")]
    UnsupportedFeaturesTypeError(String),

    #[error("Unsupported metrics type. Metrics must be a list of Metric instances or a dictionary of key value pairs. Received: {0}")]
    UnsupportedMetricsTypeError(String),

    #[error("Unsupported status. Status must be one of: All, Pending or Processed. Received: {0}")]
    InvalidStatusError(String),

    #[error("Failed to supply either input or response for the llm record")]
    MissingInputOrResponse,

    #[error("Invalid context type. Context must be a PyDict or a Pydantic BaseModel")]
    MustBeDictOrBaseModel,

    #[error("Failed to check if the context is a Pydantic BaseModel. Error: {0}")]
    FailedToCheckPydanticModel(String),

    #[error("Failed to import pydantic module. Error: {0}")]
    FailedToImportPydantic(String),
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

    #[error("Failed to supply either input or response for the llm record")]
    MissingInputOrResponse,
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

    #[error("Missing evaluation workflow")]
    MissingWorkflowError,

    #[error("Invalid argument for workflow. Argument must be a Workflow object")]
    InvalidWorkflowType,

    #[error(transparent)]
    AgentError(#[from] potato_head::AgentError),

    #[error(transparent)]
    WorkflowError(#[from] potato_head::WorkflowError),

    #[error("Invalid metric name found: {0}")]
    InvalidMetricNameError(String),

    #[error("No metrics provided for workflow validation")]
    EmptyMetricsList,

    #[error("LLM Metric requires at least one bound parameter")]
    NeedAtLeastOneBoundParameterError(String),

    #[error(
        "Missing prompt in LLM Metric. If providing a list of metrics, prompt must be present"
    )]
    MissingPromptError(String),

    #[error("No tasks found in the workflow when validating: {0}")]
    NoTasksFoundError(String),

    #[error(
        "Invalid prompt response type. Expected Score as the output type for the LLMDriftMetric prompt. Id: {0}"
    )]
    InvalidResponseType(String),

    #[error("No metrics found for the output task: {0}")]
    MetricNotFoundForOutputTask(String),

    #[error("Metric not found in profile LLM metrics: {0}")]
    MetricNotFound(String),

    #[error(transparent)]
    PotatoTypeError(#[from] potato_head::TypeError),
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
