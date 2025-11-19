use thiserror::Error;

#[derive(Error, Debug)]
pub enum StateError {
    #[error("Failed to create runtime: {0}")]
    RuntimeError(#[source] std::io::Error),
}
