use thiserror::Error;

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("{0}")]
    GrpcError(String),

    #[error(transparent)]
    HttpError(#[from] scouter_http::error::ClientError),

    #[error("Unauthorized")]
    Unauthorized,
}
