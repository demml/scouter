use scouter_dataframe::error::DataFrameError;
use scouter_types::error::ProfileError;
use scouter_types::error::RecordError;
use sqlx::Error as SqlxError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SqlError {
    #[error(transparent)]
    SqlxError(#[from] SqlxError),

    #[error("Failed to run migrations")]
    MigrateError(#[from] sqlx::migrate::MigrateError),

    #[error(transparent)]
    RecordError(#[from] RecordError),

    #[error(transparent)]
    TypeError(#[from] scouter_types::error::TypeError),

    #[error("Invalid record type: {0}")]
    InvalidRecordTypeError(String),

    #[error(transparent)]
    SemverError(#[from] semver::Error),

    #[error(transparent)]
    VersionError(#[from] scouter_semver::error::VersionError),

    #[error("Begin datetime must be before end datetime")]
    InvalidDateRangeError,

    #[error(transparent)]
    DataFrameError(#[from] DataFrameError),

    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),

    #[error(transparent)]
    CronError(#[from] cron::error::Error),

    #[error("Failed to get next run for cron schedule")]
    GetNextRunError,

    #[error("Empty batch of records")]
    EmptyBatchError,

    #[error("Record batch type is not supported")]
    UnsupportedBatchTypeError,

    #[error(transparent)]
    ProfileError(#[from] ProfileError),

    #[error(transparent)]
    OutOfRangeError(#[from] chrono::OutOfRangeError),
}
