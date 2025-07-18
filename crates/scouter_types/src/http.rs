use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct JwtToken {
    pub token: String,
}

#[derive(Debug, Clone)]
pub enum RequestType {
    Get,
    Post,
    Put,
    Delete,
}

#[derive(Debug, Clone)]
pub enum Routes {
    AuthLogin,
    AuthRefresh,
    Drift,
    SpcDrift,
    PsiDrift,
    CustomDrift,
    LLMDrift,
    Profile,
    ProfileStatus,
    Alerts,
    DownloadProfile,
    Healthcheck,
}

impl Routes {
    pub fn as_str(&self) -> &str {
        match self {
            Routes::AuthLogin => "auth/login",
            Routes::AuthRefresh => "auth/refresh",
            Routes::Profile => "profile",
            Routes::Drift => "drift",
            Routes::SpcDrift => "drift/spc",
            Routes::PsiDrift => "drift/psi",
            Routes::CustomDrift => "drift/custom",
            Routes::LLMDrift => "drift/llm",
            Routes::ProfileStatus => "profile/status",
            Routes::DownloadProfile => "profile/download",
            Routes::Alerts => "alerts",
            Routes::Healthcheck => "healthcheck",
        }
    }
}
