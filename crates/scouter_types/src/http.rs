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
    GenAITaskDrift,
    GenAIWorkflowDrift,
    Profile,
    ProfileStatus,
    Alerts,
    DownloadProfile,
    Healthcheck,
    Message,
    PaginatedTraces,
    TraceBaggage,
    TraceSpans,
    TraceSpanTags,
    RefreshTraceSummary,
    TraceMetrics,
    Tags,
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
            Routes::GenAITaskDrift => "drift/genai/task",
            Routes::GenAIWorkflowDrift => "drift/genai/workflow",
            Routes::ProfileStatus => "profile/status",
            Routes::DownloadProfile => "profile/download",
            Routes::Alerts => "alerts",
            Routes::Healthcheck => "healthcheck",
            Routes::Message => "message",
            Routes::PaginatedTraces => "trace/paginated",
            Routes::TraceBaggage => "trace/baggage",
            Routes::TraceSpans => "trace/spans",
            Routes::TraceSpanTags => "trace/spans/tags",
            Routes::RefreshTraceSummary => "trace/refresh-summary",
            Routes::TraceMetrics => "trace/metrics",
            Routes::Tags => "tags",
        }
    }
}
