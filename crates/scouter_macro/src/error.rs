use thiserror::Error;

#[derive(Error, Debug)]
pub enum MacroError {
    #[error("Error: {0}")]
    Error(String),

    #[error("Failed to parse macro input: {0}")]
    ParseError(String),

    #[error("Unsupported response type")]
    ResponseTypeNotSupported,
}
