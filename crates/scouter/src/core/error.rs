use thiserror::Error;

#[derive(Error, Debug)]
pub enum AlertError {
    #[error("Failed to create alert: {0}")]
    CreateError(String),
}

#[derive(Error, Debug)]
pub enum MonitorError {
    #[error("Failed to create monitor: {0}")]
    CreateError(String),

    #[error("Failed to sample data: {0}")]
    SampleDataError(String),

    #[error("Compute error: {0}")]
    ComputeError(String),
}
