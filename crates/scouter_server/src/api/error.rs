use thiserror::Error;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error(transparent)]
    SqlxError(#[from] sqlx::Error),

    #[error(transparent)]
    SqlError(#[from] scouter_sql::sql::error::SqlError),

    #[error(transparent)]
    DataFrameError(#[from] scouter_dataframe::error::DataFrameError),

    #[error("Failed to get entities to archive")]
    GetEntitiesToArchiveError(#[source] scouter_sql::sql::error::SqlError),

    #[error("Failed to get data to archive")]
    GetDataToArchiveError(#[source] scouter_sql::sql::error::SqlError),

    #[error("Failed to update data to archived")]
    UpdateDataToArchivedError(#[source] scouter_sql::sql::error::SqlError),

    #[error("No Profile found")]
    NoProfileFoundError,

    #[error(transparent)]
    DriftError(#[from] scouter_drift::error::DriftError),

    #[error(transparent)]
    RecordError(#[from] scouter_types::error::RecordError),
}
