use scouter_types::RecordType;
use thiserror::Error;
use tracing::span::Record;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error(transparent)]
    DecodeError(#[from] base64::DecodeError),

    #[error(transparent)]
    UtilError(#[from] scouter_types::error::UtilError),

    #[error("Failed to convert string to Utf-8: {0}")]
    ConvertUtf8Error(String),

    #[error(transparent)]
    ObjectStorageError(#[from] object_store::Error),

    #[error(transparent)]
    ParseError(#[from] url::ParseError),
}

#[derive(Error, Debug)]
pub enum DataFrameError {
    #[error("Failed to read batch: {0}")]
    ReadBatchError(String),

    #[error("Failed to create batch: {0}")]
    CreateBatchError(String),

    #[error(transparent)]
    StorageError(#[from] StorageError),

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

    #[error("Downcast error: {0}")]
    DowncastError(String),

    #[error("Failed to get column: {0}")]
    GetColumnError(String),

    #[error("Missing field: {0}")]
    MissingFieldError(String),

    #[error(transparent)]
    DatafusionError(#[from] datafusion::error::DataFusionError),

    #[error(transparent)]
    RecordError(#[from] scouter_types::error::RecordError),

    #[error(transparent)]
    ArrowError(#[from] arrow::error::ArrowError),

    #[error("Invalid recor type: {0}")]
    InvalidRecordTypeError(&'static RecordType),
}
