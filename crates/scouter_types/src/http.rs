use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
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
    AgentTaskDrift,
    AgentWorkflowDrift,
    Profile,
    ProfileStatus,
    Alerts,
    DownloadProfile,
    Healthcheck,
    Message,
    PaginatedTraces,
    TraceBaggage,
    TraceSpans,
    TraceSpanFilters,
    TraceSpanTags,
    RefreshTraceSummary,
    TraceMetrics,
    Tags,
    EvalScenarios,
    GenAiTokenMetrics,
    GenAiOperationBreakdown,
    GenAiModelUsage,
    GenAiAgentActivity,
    GenAiToolActivity,
    GenAiErrorBreakdown,
    GenAiSpans,
    GenAiConversation,
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
            Routes::AgentTaskDrift => "drift/agent/task",
            Routes::AgentWorkflowDrift => "drift/agent/workflow",
            Routes::ProfileStatus => "profile/status",
            Routes::DownloadProfile => "profile/download",
            Routes::Alerts => "alerts",
            Routes::Healthcheck => "healthcheck",
            Routes::Message => "message",
            Routes::PaginatedTraces => "trace/paginated",
            Routes::TraceBaggage => "trace/baggage",
            Routes::TraceSpans => "trace/spans",
            Routes::TraceSpanFilters => "trace/spans/filters",
            Routes::TraceSpanTags => "trace/spans/tags",
            Routes::RefreshTraceSummary => "trace/refresh-summary",
            Routes::TraceMetrics => "trace/metrics",
            Routes::Tags => "tags",
            Routes::EvalScenarios => "eval/scenarios",
            Routes::GenAiTokenMetrics => "genai/metrics/tokens",
            Routes::GenAiOperationBreakdown => "genai/metrics/operations",
            Routes::GenAiModelUsage => "genai/metrics/models",
            Routes::GenAiAgentActivity => "genai/metrics/agents",
            Routes::GenAiToolActivity => "genai/metrics/tools",
            Routes::GenAiErrorBreakdown => "genai/metrics/errors",
            Routes::GenAiSpans => "genai/spans",
            Routes::GenAiConversation => "genai/conversation",
        }
    }
}
