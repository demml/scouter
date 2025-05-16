use password_auth::VerifyError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Invalid username provided")]
    InvalidUser,

    #[error("Invalid password provided")]
    InvalidPassword(#[source] VerifyError),

    #[error("Session timeout for user occured")]
    SessionTimeout,

    #[error("JWT token provided is invalid")]
    InvalidJwtToken,

    #[error("Refresh token is invalid")]
    InvalidRefreshToken,
}
