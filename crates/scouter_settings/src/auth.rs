use serde::Serialize;

#[derive(Debug, Clone, Default, Serialize)]
pub struct AuthSettings {
    pub jwt_secret: String,
    pub refresh_secret: String,
}
