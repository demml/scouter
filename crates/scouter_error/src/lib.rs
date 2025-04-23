use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::PyErr;
use serde::Deserialize;
use std::fmt::Display;
use thiserror::Error;
use tracing::error;

pub trait TracedError: Display {
    fn trace(&self) {
        error!("{}", self);
    }
}

#[derive(Error, Debug)]
pub enum ScouterTypeError {
    #[error("Failed to construct TimeInterval {0}")]
    TimeIntervalError(String),
}

// add tracing trait to ScouterError

#[derive(Error, Debug, Deserialize, PartialEq)]
pub enum UtilError {
    #[error("Failed to parse cron expression: {0}")]
    ParseCronError(String),

    #[error("Failed to serialize: {0}")]
    SerializeError(String),

    #[error("Failed to deserialize: {0}")]
    DeSerializeError(String),

    #[error("Failed to decode base64-encoded string: {0}")]
    DecodeBase64Error(String),

    #[error("Failed to convert string to Utf-8: {0}")]
    ConvertUtf8Error(String),

    #[error("Failed to set log level: {0}")]
    SetLogLevelError(String),

    #[error("Failed to get parent path")]
    GetParentPathError,

    #[error("Failed to create directory")]
    CreateDirectoryError,

    #[error("Failed to write to file")]
    WriteError,

    #[error("Failed to read to file")]
    ReadError,
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

    pub fn traced_deserialize_error(err: impl Display) -> Self {
        let error = Self::DeSerializeError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_set_log_level_error(err: impl Display) -> Self {
        let error = Self::SetLogLevelError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_decode_base64_error(err: impl Display) -> Self {
        let error = Self::DecodeBase64Error(err.to_string());
        error.trace();
        error
    }

    pub fn traced_convert_utf8_error(err: impl Display) -> Self {
        let error = Self::ConvertUtf8Error(err.to_string());
        error.trace();
        error
    }
}

#[derive(Error, Debug, Deserialize, PartialEq)]
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

    #[error(transparent)]
    UtilError(#[from] UtilError),
}

#[derive(Error, Debug)]
pub enum ProfilerError {
    #[error("Quantile error: {0}")]
    QuantileError(String),

    #[error("Error calculating mean")]
    MeanError,

    #[error("Compute error: {0}")]
    ComputeError(String),

    #[error("Failed to compute string statistics: {0}")]
    StringStatsError(String),

    #[error("Failed to create feature map: {0}")]
    FeatureMapError(String),

    #[error("Failed to convert: {0}")]
    ConversionError(String),

    #[error("Failed to create string profile: {0}")]
    StringProfileError(String),

    // array concatenation error
    #[error("Failed to concatenate arrays: {0}")]
    ConcatenateError(String),
}

impl TracedError for ProfilerError {}

impl ProfilerError {
    pub fn traced_string_stats_error(err: impl Display) -> Self {
        let error = Self::StringStatsError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_concatenate_error(err: impl Display) -> Self {
        let error = Self::ConcatenateError(err.to_string());
        error.trace();
        error
    }
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

#[derive(Error, Debug, PartialEq)]
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

    #[error(transparent)]
    DataFrameError(#[from] DataFrameError),

    #[error("Failed to extract value {0} {1}")]
    FailedToExtractError(String, String),

    #[error("Invalid date range: {0}")]
    InvalidDateRangeError(String),

    #[error("Failed to convert dataframe: {0}")]
    FailedToConvertDataFrameError(String),
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

    pub fn traced_connection_error(err: impl Display) -> Self {
        let error = Self::ConnectionError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_failed_to_convert_dataframe_error(err: impl Display) -> Self {
        let error = Self::FailedToConvertDataFrameError(err.to_string());
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

#[derive(Error, Debug, PartialEq)]
pub enum DataFrameError {
    #[error("Failed to read batch: {0}")]
    ReadBatchError(String),

    #[error("Failed to create batch: {0}")]
    CreateBatchError(String),

    #[error(transparent)]
    StorageError(#[from] StorageError),

    #[error(transparent)]
    DriftError(#[from] DriftError),

    #[error("Failed to add year column: {0}")]
    AddYearColumnError(String),

    #[error("Failed to add month column: {0}")]
    AddMonthColumnError(String),

    #[error("Failed to add day column: {0}")]
    AddDayColumnError(String),

    #[error("Failed to add hour column: {0}")]
    AddHourColumnError(String),

    #[error("Failed to write to parquet: {0}")]
    WriteParquetError(String),

    #[error("Failed to parse table path: {0}")]
    ParseTablePathError(String),

    #[error("Failed to infer schema: {0}")]
    InferSchemaError(String),

    #[error("Failed to create listing table: {0}")]
    CreateListingTableError(String),

    #[error("Failed to register table: {0}")]
    RegisterTableError(String),

    #[error("Invalid record type: {0}")]
    InvalidRecordTypeError(String),

    #[error("Downcast error: {0}")]
    DowncastError(String),

    #[error("Failed to get column: {0}")]
    GetColumnError(String),

    #[error("Missing field: {0}")]
    MissingFieldError(String),
}

impl TracedError for DataFrameError {}

impl DataFrameError {
    pub fn traced_read_batch_error(err: impl Display) -> Self {
        let error = Self::ReadBatchError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_create_batch_error(err: impl Display) -> Self {
        let error = Self::CreateBatchError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_add_year_column_error(err: impl Display) -> Self {
        let error = Self::AddYearColumnError(err.to_string());
        error.trace();
        error
    }
    pub fn traced_add_month_column_error(err: impl Display) -> Self {
        let error = Self::AddMonthColumnError(err.to_string());
        error.trace();
        error
    }
    pub fn traced_add_day_column_error(err: impl Display) -> Self {
        let error = Self::AddDayColumnError(err.to_string());
        error.trace();
        error
    }
    pub fn traced_add_hour_column_error(err: impl Display) -> Self {
        let error = Self::AddHourColumnError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_write_parquet_error(err: impl Display) -> Self {
        let error = Self::WriteParquetError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_parse_table_path_error(err: impl Display) -> Self {
        let error = Self::ParseTablePathError(err.to_string());
        error.trace();
        error
    }
    pub fn traced_infer_schema_error(err: impl Display) -> Self {
        let error = Self::InferSchemaError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_create_listing_table_error(err: impl Display) -> Self {
        let error = Self::CreateListingTableError(err.to_string());
        error.trace();
        error
    }
    pub fn traced_register_table_error(err: impl Display) -> Self {
        let error = Self::RegisterTableError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_invalid_record_type_error(err: impl Display) -> Self {
        let error = Self::InvalidRecordTypeError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_downcast_error(err: impl Display) -> Self {
        let error = Self::DowncastError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_get_column_error(err: impl Display) -> Self {
        let error = Self::GetColumnError(err.to_string());
        error.trace();
        error
    }
}

#[derive(Error, Debug, PartialEq)]
pub enum DriftError {
    #[error("Error: {0}")]
    Error(String),

    #[error("Failed to create rule. Rule must be of length 8")]
    RuleLengthError,

    #[error(transparent)]
    ParseIntError(#[from] std::num::ParseIntError),

    #[error("Failed to compute - {0}")]
    ComputeError(String),

    #[error("Failed to shape array: {0}")]
    ShapeError(String),

    #[error("Missing feature {0}")]
    MissingFeatureError(String),

    #[error("Features missing from feature map")]
    MissingFeaturesError,

    #[error("Failed to sample data {0}")]
    SampleDataError(String),

    #[error("Failed to set control value: {0}")]
    SetControlValueError(String),

    #[error("Features and array are not the same length")]
    FeatureArrayLengthError,

    #[error("Failed to create bins - {0}")]
    CreateBinsError(String),

    #[error(
        "Feature mismatch, feature '{0}' not found. Available features in the drift profile: {1}"
    )]
    FeatureMismatchError(String, String),

    // array concantenation error
    #[error("Failed to concatenate arrays: {0}")]
    ConcatenateError(String),

    // invalid config
    #[error("Invalid config: {0}")]
    InvalidConfigError(String),

    // invalid drift type
    #[error("Invalid drift type")]
    InvalidDriftTypeError,
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

    pub fn traced_compute_error(err: impl Display) -> Self {
        let error = Self::ComputeError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_shape_error(err: impl Display) -> Self {
        let error = Self::ShapeError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_missing_feature_error(err: impl Display) -> Self {
        let error = Self::MissingFeatureError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_sample_data_error(err: impl Display) -> Self {
        let error = Self::SampleDataError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_set_control_value_error(err: impl Display) -> Self {
        let error = Self::SetControlValueError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_feature_length_error() -> Self {
        let error = Self::FeatureArrayLengthError;
        error.trace();
        error
    }

    pub fn traced_missing_features_error() -> Self {
        let error = Self::MissingFeaturesError;
        error.trace();
        error
    }

    pub fn traced_create_bins_error(err: impl Display) -> Self {
        let error = Self::CreateBinsError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_feature_mismatch_error(feature: impl Display, available: impl Display) -> Self {
        let error = Self::FeatureMismatchError(feature.to_string(), available.to_string());
        error.trace();
        error
    }
}

#[derive(Error, Debug, PartialEq)]
pub enum ClientError {
    #[error("Failed to get JWT token: {0}")]
    FailedToGetJwtToken(String),

    #[error("Failed to parse JWT token: {0}")]
    FailedToParseJwtToken(String),

    #[error("Failed to send request: {0}")]
    FailedToSendRequest(String),

    #[error("Failed to get response: {0}")]
    FailedToGetResponse(String),

    #[error("Failed to serialize: {0}")]
    FailedToSerialize(String),

    #[error("Failed to deserialize: {0}")]
    FailedToDeserialize(String),

    #[error("Failed to create header: {0}")]
    FailedToCreateHeader(String),

    #[error("Failed to create client: {0}")]
    FailedToCreateClient(String),

    #[error("Unauthorized")]
    Unauthorized,

    #[error(transparent)]
    SqlError(#[from] SqlError),
}

impl TracedError for ClientError {}

impl ClientError {
    pub fn traced_jwt_error(err: impl Display) -> Self {
        let error = ClientError::FailedToGetJwtToken(err.to_string());
        error.trace();
        error
    }

    pub fn traced_parse_jwt_error(err: impl Display) -> Self {
        let error = ClientError::FailedToParseJwtToken(err.to_string());
        error.trace();
        error
    }

    pub fn traced_request_error(err: impl Display) -> Self {
        let error = ClientError::FailedToSendRequest(err.to_string());
        error.trace();
        error
    }

    pub fn traced_unauthorized_error() -> Self {
        let error = ClientError::Unauthorized;
        error.trace();
        error
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
}

#[derive(Error, Debug, PartialEq)]
pub enum EventError {
    #[error("Failed to connect: {0}")]
    ConnectionError(String),

    #[error("Failed to create channel: {0}")]
    ChannelError(String),

    #[error("Failed to setup queue: {0}")]
    DeclareQueueError(String),

    #[error("Failed to publish message: {0}")]
    PublishError(String),

    #[error("Failed to consume message: {0}")]
    ConsumeError(String),

    #[error("Failed to flush message: {0}")]
    FlushError(String),

    #[error("Failed to send message: {0}")]
    SendError(String),

    #[error(transparent)]
    UtilError(#[from] UtilError),

    #[error(transparent)]
    ClientError(#[from] ClientError),

    #[error(transparent)]
    SqlError(#[from] SqlError),

    #[error("Invalid compression type")]
    InvalidCompressionTypeError,

    #[error("Subscribe error: {0}")]
    SubscribeError(String),

    #[error("Failed to setup qos: {0}")]
    SetupQosError(String),

    #[error("Failed to setup consumer: {0}")]
    SetupConsumerError(String),
}

impl TracedError for EventError {}

impl EventError {
    pub fn traced_connection_error(err: impl Display) -> Self {
        let error = Self::ConnectionError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_channel_error(err: impl Display) -> Self {
        let error = Self::ChannelError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_declare_queue_error(err: impl Display) -> Self {
        let error = Self::DeclareQueueError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_publish_error(err: impl Display) -> Self {
        let error = Self::PublishError(err.to_string());
        error.trace();
        error
    }
    pub fn traced_consume_error(err: impl Display) -> Self {
        let error = Self::ConsumeError(err.to_string());
        error.trace();
        error
    }
    pub fn traced_flush_error(err: impl Display) -> Self {
        let error = Self::FlushError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_send_error(err: impl Display) -> Self {
        let error = Self::SendError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_subscribe_error(err: impl Display) -> Self {
        let error = Self::SubscribeError(err.to_string());
        error.trace();
        error
    }

    pub fn traced_setup_qos_error(err: impl Display) -> Self {
        let error = Self::SetupQosError(err.to_string());
        error.trace();
        error
    }
}

/// THis should be the top-level error that all other errors are converted to
#[derive(Error, Debug)]
pub enum ScouterError {
    #[error("Failed to serialize string")]
    SerializeError,

    #[error("Failed to deserialize string")]
    DeSerializeError,

    #[error("Failed to create path")]
    CreatePathError,

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

    #[error(transparent)]
    ProfileError(#[from] ProfilerError),

    #[error(transparent)]
    DataFrameError(#[from] DataFrameError),

    #[error(transparent)]
    EventError(#[from] EventError),

    #[error(transparent)]
    UtilError(#[from] UtilError),

    #[error(transparent)]
    ScouterTypeError(#[from] ScouterTypeError),

    #[error("Missing value in map")]
    MissingValue,

    #[error("Empty ServerRecordsError")]
    EmptyServerRecordsError,

    // rabbitmq

    // downcast error
    #[error("Failed to downcast: {0}")]
    FailedToDowncast(String),

    // unsupported data type
    #[error("Unsupported data type: {0}")]
    UnsupportedDataType(String),

    // data is not numpy
    #[error("Data is not a numpy array")]
    DataNotNumpy,

    // column names must be strings
    #[error("Column names must be string type")]
    ColumnNamesMustBeStrings,
}

impl TracedError for ScouterError {}

// add a raise method so that each error is traced before being returned
impl ScouterError {
    pub fn raise<T>(self) -> Result<T, Self> {
        self.trace();
        Err(self)
    }

    pub fn traced_downcast_error(err: impl Display) -> Self {
        let error = Self::FailedToDowncast(err.to_string());
        error.trace();
        error
    }

    pub fn traced_unsupported_data_type_error(err: impl Display) -> Self {
        let error = Self::UnsupportedDataType(err.to_string());
        error.trace();
        error
    }

    pub fn traced_data_not_numpy_error() -> Self {
        let error = Self::DataNotNumpy;
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
