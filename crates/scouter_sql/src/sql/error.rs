use sqlx::Error as SqlxError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SqlError {
    #[error(transparent)]
    SqlxError(#[from] SqlxError),
}
