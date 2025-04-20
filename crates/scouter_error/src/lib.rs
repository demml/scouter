use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::PyErr;
use serde::Deserialize;
use std::fmt::Display;
use thiserror::Error;
use tracing::error;
// add tracing trait to ScouterError
pub trait TracedError: Display {
    fn trace(&self) {
        error!("{}", self);
    }
}

#[derive(Error, Debug, Deserialize)]
pub enum UtilError {
    #[error("Failed to parse cron expression: {0}")]
    ParseCronError(String),

    #[error("Failed to serialize: {0}")]
    SerializeError(String),
}

impl TracedError for UtilError {}

impl UtilError {
    pub fn traced_parse_cron_error(err: impl Display) -> Self {
        let error = Self::ParseCronError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_serialize_error(err: impl Display) -> Self {
        let error = Self::SerializeError(err.to_string());
        error.trace();
        error
    }
}

#[derive(Error, Debug, Deserialize)]
pub enum StorageError {
    #[error("Failed to create object store: {0}")]
    ObjectStoreError(String),

    #[error("Failed to create storage: {0}")]
    StorageError(String),

    #[error("Failed to create file system: {0}")]
    FileSystemError(String),

    #[error("Failed to create file: {0}")]
    FileError(String),

    #[error("Failed to create directory: {0}")]
    DirectoryError(String),

    #[error("Failed to write to file: {0}")]
    WriteError(String),

    #[error("Failed to read from file: {0}")]
    ReadError(String),
}

#[derive(Error, Debug, PartialEq, Deserialize)]
pub enum MonitorError {
    #[error("{0}")]
    CreateError(String),

    #[error("Sample error: {0}")]
    SampleDataError(String),

    #[error("Compute error: {0}")]
    ComputeError(String),

    #[error("Shape mismatch: {0}")]
    ShapeMismatchError(String),

    #[error("Missing feature: {0}")]
    MissingFeatureError(String),

    #[error("Array Error: {0}")]
    ArrayError(String),
}

#[derive(Error, Debug)]
pub enum ProfilerError {
    #[error("Quantile error: {0}")]
    QuantileError(String),

    #[error("Error calculating mean")]
    MeanError,

    #[error("Compute error: {0}")]
    ComputeError(String),

    #[error("Failed to compute string statistics")]
    StringStatsError,

    #[error("Failed to create feature map: {0}")]
    FeatureMapError(String),

    #[error("Array Error: {0}")]
    ArrayError(String),

    #[error("Failed to convert: {0}")]
    ConversionError(String),

    #[error("Failed to create string profile: {0}")]
    StringProfileError(String),
}

#[derive(Error, Debug, Deserialize)]
pub enum FeatureQueueError {
    #[error("{0}")]
    InvalidFormatError(String),

    #[error("Failed to create drift record: {0}")]
    DriftRecordError(String),

    #[error("Failed to create alert record: {0}")]
    AlertRecordError(String),

    #[error("Failed to get feature")]
    GetFeatureError,

    #[error("Missing feature map")]
    MissingFeatureMapError,

    #[error("invalid data type detected for feature: {0}")]
    InvalidFeatureTypeError(String),

    #[error("invalid value detected for feature: {0}, error: {1}")]
    InvalidValueError(String, String),

    #[error("Failed to get bin given bin id")]
    GetBinError,
}

// impl From for PyErr

impl From<FeatureQueueError> for PyErr {
    fn from(err: FeatureQueueError) -> PyErr {
        PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(err.to_string())
    }
}

#[derive(Error, Debug, Deserialize)]
pub enum SqlError {
    #[error("Failed to run sql migrations: {0}")]
    MigrationError(String),

    #[error("Failed to run sql query: {0}")]
    QueryError(String),

    #[error("Failed to parse version: {0}")]
    VersionError(String),

    #[error("File error: {0}")]
    FileError(String),

    #[error("Failed to connect to the database - {0}")]
    ConnectionError(String),

    #[error("Failed to update drift profile: {0}")]
    UpdateDriftProfileError(String),

    #[error("Failed to get bin proportions: {0}")]
    GetBinProportionsError(String),

    #[error("Failed to get custom metrics: {0}")]
    GetCustomMetricsError(String),

    #[error("Failed to get insert metrics: {0}")]
    InsertCustomMetricsError(String),

    #[error("Invalid record type: {0}")]
    InvalidRecordTypeError(String),

    #[error("Failed to get entities: {0}")]
    GetEntitiesError(String),

    #[error("Failed to get entity data: {0}")]
    GetEntityDataError(String),

    #[error("Failed to get features: {0}")]
    GetFeaturesError(String),

    #[error("Failed to get next run: {0}")]
    GetNextRunError(String),

    #[error("Failed to get drift task: {0}")]
    GetDriftTaskError(String),

    #[error(transparent)]
    UtilError(#[from] UtilError),

    #[error("Failed to extract value {0} {1}")]
    FailedToExtractError(String, String),
}

impl TracedError for SqlError {}

impl SqlError {
    pub fn traced_query_error(err: impl Display) -> Self {
        let error = Self::QueryError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_update_drift_profile_error(err: impl Display) -> Self {
        let error = Self::UpdateDriftProfileError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_get_bin_proportions_error(err: impl Display) -> Self {
        let error = Self::GetBinProportionsError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_get_custom_metrics_error(err: impl Display) -> Self {
        let error = Self::GetCustomMetricsError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_insert_custom_metrics_error(err: impl Display) -> Self {
        let error = Self::InsertCustomMetricsError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_invalid_record_type_error(err: impl Display) -> Self {
        let error = Self::InvalidRecordTypeError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_get_entities_error(err: impl Display) -> Self {
        let error = Self::GetEntitiesError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_get_entity_data_error(err: impl Display) -> Self {
        let error = Self::GetEntityDataError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_get_features_error(err: impl Display) -> Self {
        let error = Self::GetFeaturesError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_get_next_run_error(err: impl Display) -> Self {
        let error = Self::GetNextRunError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_get_drift_task_error(err: impl Display) -> Self {
        let error = Self::GetDriftTaskError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_failed_to_extract_error(err: impl Display, field: impl Display) -> Self {
        let error = Self::FailedToExtractError(field.to_string(), err.to_string());
        error.trace();
        error
    }
}

#[derive(Error, Debug, Deserialize)]
pub enum AlertError {
    #[error("Error: {0}")]
    GeneralError(String),

    #[error("Failed to create alert: {0}")]
    CreateError(String),

    #[error("{0}")]
    DriftError(String),
}

#[derive(Error, Debug)]
pub enum DriftError {
    #[error("Error: {0}")]
    Error(String),

    #[error("Failed to create rule. Rule must be of length 8")]
    RuleLengthError,

    #[error(transparent)]
    ParseIntError(#[from] std::num::ParseIntError),

    #[error(transparent)]
    SqlError(#[from] SqlError),
}

impl TracedError for DriftError {}

impl DriftError {
    pub fn raise<T>(self) -> Result<T, Self> {
        self.trace();
        Err(self)
    }

    pub fn traced_rule_length_error() -> Self {
        let error = Self::RuleLengthError;
        error.trace();
        error
    }
}

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("Failed to get JWT token: {0}")]
    FailedToGetJwtToken(String),

    #[error("Failed to parse JWT token: {0}")]
    FailedToParseJwtToken(String),

    #[error("Failed to send request: {0}")]
    FailedToSendRequest(String),

    #[error("Failed to get response: {0}")]
    FailedToGetResponse(String),

    #[error("Unauthorized")]
    Unauthorized,

    #[error(transparent)]
    SqlError(#[from] SqlError),
}

impl TracedError for ClientError {}

/// THis should be the top-level error that all other errors are converted to
/// This should be the error that is returned to the user
#[derive(Error, Debug)]
pub enum ScouterError {
    #[error("Failed to serialize string")]
    SerializeError,

    #[error("Failed to deserialize string")]
    DeSerializeError,

    #[error("Failed to create path")]
    CreatePathError,

    #[error("Failed to get parent path")]
    GetParentPathError,

    #[error("Failed to create directory")]
    CreateDirectoryError,

    #[error("Failed to write to file")]
    WriteError,

    #[error("Failed to read to file")]
    ReadError,

    #[error("Type error for {0}")]
    TypeError(String),

    #[error("Missing feature map")]
    MissingFeatureMapError,

    #[error("Failed to create string profile: {0}")]
    StringProfileError(String),

    #[error("Invalid drift type: {0}")]
    InvalidDriftTypeError(String),

    #[error("Shape mismatch: {0}")]
    ShapeMismatchError(String),

    #[error("{0}")]
    FeatureError(String),

    #[error("{0}")]
    Error(String),

    #[error(transparent)]
    MonitorError(#[from] MonitorError),

    #[error(transparent)]
    AlertError(#[from] AlertError),

    #[error(transparent)]
    CustomError(#[from] CustomMetricError),

    #[error(transparent)]
    FeatureQueueError(#[from] FeatureQueueError),

    #[error(transparent)]
    SqlError(#[from] SqlError),

    #[error(transparent)]
    DriftError(#[from] DriftError),

    #[error(transparent)]
    StorageError(#[from] StorageError),

    #[error(transparent)]
    ClientError(#[from] ClientError),

    #[error("Missing value in map")]
    MissingValue,

    #[error("Empty ServerRecordsError")]
    EmptyServerRecordsError,

    #[error("Failed to serialize: {0}")]
    FailedToSerialize(String),

    #[error("Failed to deserialize: {0}")]
    FailedToDeserialize(String),

    #[error("Failed to create header: {0}")]
    FailedToCreateHeader(String),

    #[error("Failed to create client: {0}")]
    FailedToCreateClient(String),

    // rabbitmq
    #[error("Failed to connect to RabbitMQ: {0}")]
    FailedToConnectRabbitMQ(String),

    #[error("Failed to declare queue: {0}")]
    FailedToDeclareQueue(String),

    #[error("Failed to setup QoS: {0}")]
    FailedToSetupQos(String),

    #[error("Failed to connect to Kafka: {0}")]
    FailedToConnectKafka(String),

    #[error("Failed to consume queue: {0}")]
    FailedToConsumeQueue(String),

    #[error("Failed to subscribe to topic: {0}")]
    FailedToSubscribeTopic(String),
}

impl TracedError for ScouterError {}

// add a raise method so that each error is traced before being returned
impl ScouterError {
    pub fn raise<T>(self) -> Result<T, Self> {
        self.trace();
        Err(self)
    }

    pub fn traced_jwt_error(err: impl Display) -> Self {
        let error = ClientError::FailedToGetJwtToken(err.to_string());
        error.trace();
        error.into()
    }

    pub fn traced_parse_jwt_error(err: impl Display) -> Self {
        let error = ClientError::FailedToParseJwtToken(err.to_string());
        error.trace();
        error.into()
    }

    pub fn traced_request_error(err: impl Display) -> Self {
        let error = ClientError::FailedToSendRequest(err.to_string());
        error.trace();
        error.into()
    }

    pub fn traced_unauthorized_error() -> Self {
        let error = ClientError::Unauthorized;
        error.trace();
        error.into()
    }

    pub fn traced_serialize_error(err: impl Display) -> Self {
        let error = Self::FailedToSerialize(err.to_string());
        error.trace();
        error
    }

    pub fn traced_deserialize_error(err: impl Display) -> Self {
        let error = Self::FailedToDeserialize(err.to_string());
        error.trace();
        error
    }

    pub fn traced_create_header_error(err: impl Display) -> Self {
        let error = Self::FailedToCreateHeader(err.to_string());
        error.trace();
        error
    }

    pub fn traced_create_client_error(err: impl Display) -> Self {
        let error = Self::FailedToCreateClient(err.to_string());
        error.trace();
        error
    }

    pub fn traced_connect_rabbitmq_error(err: impl Display) -> Self {
        let error = Self::FailedToConnectRabbitMQ(err.to_string());
        error.trace();
        error
    }

    pub fn traced_setup_qos_error(err: impl Display) -> Self {
        let error = Self::FailedToSetupQos(err.to_string());
        error.trace();
        error
    }

    pub fn traced_connect_kafka_error(err: impl Display) -> Self {
        let error = Self::FailedToConnectKafka(err.to_string());
        error.trace();
        error
    }

    pub fn traced_declare_queue_error(err: impl Display) -> Self {
        let error = Self::FailedToDeclareQueue(err.to_string());
        error.trace();
        error
    }

    pub fn traced_consume_queue_error(err: impl Display) -> Self {
        let error = Self::FailedToConsumeQueue(err.to_string());
        error.trace();
        error
    }

    pub fn traced_subscribe_topic_error(err: impl Display) -> Self {
        let error = Self::FailedToSubscribeTopic(err.to_string());
        error.trace();
        error
    }
}

// impl From for PyErr

impl From<std::io::Error> for ScouterError {
    fn from(err: std::io::Error) -> ScouterError {
        ScouterError::Error(err.to_string())
    }
}

impl From<PyErr> for ScouterError {
    fn from(err: PyErr) -> ScouterError {
        ScouterError::Error(err.to_string())
    }
}

impl From<ScouterError> for PyErr {
    fn from(err: ScouterError) -> PyErr {
        PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(err.to_string())
    }
}

#[derive(Error, Debug)]
pub enum DispatchError {
    #[error("{0}")]
    OpsGenieError(String),

    #[error("{0}")]
    SlackError(String),

    #[error("{0}")]
    HttpError(String),

    #[error("Error processing alerts: {0}")]
    AlertProcessError(String),

    #[error("Error setting alerter: {0}")]
    AlerterError(String),
}

#[derive(Error, Debug, Deserialize)]
pub enum ObserverError {
    #[error("Route not found {0}")]
    RouteNotFound(String),

    #[error("Failed to update route metrics: {0}")]
    UpdateMetricsError(String),

    #[error("Failed to compute quantiles: {0}")]
    QuantileError(String),

    #[error("Failed to collect metrics: {0}")]
    CollectMetricsError(String),
}

impl From<ObserverError> for PyErr {
    fn from(err: ObserverError) -> PyErr {
        PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(err.to_string())
    }
}

#[derive(Error, Debug, Deserialize)]
pub enum CustomMetricError {
    #[error("Cannot create metric profile, no metrics were provided")]
    NoMetricsError,

    #[error("{0}")]
    Error(String),
}

impl From<CustomMetricError> for PyErr {
    fn from(err: CustomMetricError) -> PyErr {
        PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(err.to_string())
    }
}

#[derive(Error, Debug, Deserialize)]
pub enum ConfigError {
    #[error("{0}")]
    Error(String),
}

#[derive(Error, Debug, Deserialize)]
pub enum LoggingError {
    #[error("{0}")]
    Error(String),
}

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Invalid username provided")]
    InvalidUser,

    #[error("Invalid password provided")]
    InvalidPassword,

    #[error("Session timeout for user occured")]
    SessionTimeout,

    #[error("JWT token provided is invalid")]
    InvalidJwtToken,

    #[error("Refresh token is invalid")]
    InvalidRefreshToken,
}

create_exception!(scouter, PyScouterError, PyException);
