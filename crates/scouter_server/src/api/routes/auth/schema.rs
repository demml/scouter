use serde::Serialize;

#[derive(Serialize)]
pub struct AuthError {
    pub error: String,
    pub message: String,
}

#[derive(Serialize)]
pub struct Authenticated {
    pub is_authenticated: bool,
}
