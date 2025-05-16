use thiserror::Error;

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
