use thiserror::Error;

#[derive(Error, Debug)]
pub enum AlertError {
    #[error("Failed to create alert: {0}")]
    CreateError(String),
}

#[derive(Error, Debug)]
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
}
