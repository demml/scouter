use thiserror::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error(transparent)]
    DecodeError(#[from] base64::DecodeError),

    #[error(transparent)]
    UtilError(#[from] scouter_types::error::UtilError),

    #[error(transparent)]
    ObjectStorageError(#[from] object_store::Error),

    #[error(transparent)]
    ParseError(#[from] url::ParseError),

    #[error(transparent)]
    Utf8Error(#[from] std::string::FromUtf8Error),
}

#[derive(Error, Debug)]
pub enum DataFrameError {
    #[error("Failed to read batch: {0}")]
    ReadBatchError(String),

    #[error("Failed to create batch: {0}")]
    CreateBatchError(String),

    #[error(transparent)]
    StorageError(#[from] StorageError),

    #[error("Failed to add year column")]
    AddYearColumnError(#[source] datafusion::error::DataFusionError),

    #[error("Failed to add month column")]
    AddMonthColumnError(#[source] datafusion::error::DataFusionError),

    #[error("Failed to add day column")]
    AddDayColumnError(#[source] datafusion::error::DataFusionError),

    #[error("Failed to add hour column")]
    AddHourColumnError(#[source] datafusion::error::DataFusionError),

    #[error("Failed to write to parquet")]
    WriteParquetError(#[source] datafusion::error::DataFusionError),

    #[error("Failed to infer schema")]
    InferSchemaError(#[source] datafusion::error::DataFusionError),

    #[error("Failed to create listing table")]
    CreateListingTableError(#[source] datafusion::error::DataFusionError),

    #[error("Failed to register table")]
    RegisterTableError(#[source] datafusion::error::DataFusionError),

    #[error("Downcast error: {0}")]
    DowncastError(&'static str),

    #[error("Failed to get column: {0}")]
    GetColumnError(&'static str),

    #[error("Missing field: {0}")]
    MissingFieldError(&'static str),

    #[error(transparent)]
    DatafusionError(#[from] datafusion::error::DataFusionError),

    #[error(transparent)]
    RecordError(#[from] scouter_types::error::RecordError),

    #[error(transparent)]
    ArrowError(#[from] arrow::error::ArrowError),

    #[error("Invalid record type provided")]
    InvalidRecordTypeError,
}
