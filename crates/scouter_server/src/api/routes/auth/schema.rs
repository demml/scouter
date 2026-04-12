use serde::Serialize;

#[derive(Serialize, utoipa::ToSchema)]
pub struct AuthError {
    pub error: String,
    pub message: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct Authenticated {
    pub is_authenticated: bool,
}
