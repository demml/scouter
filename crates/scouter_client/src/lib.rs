pub mod data_utils;
pub mod dataset;
pub mod drifter;
pub mod error;
pub mod http;
pub mod profiler;

pub use dataset::client::DatasetProducer;
pub use dataset::config::{TableConfig, WriteConfig};
pub use dataset::error::DatasetClientError;
pub use dataset::query_result::QueryResult;
pub use dataset::reader::DatasetClient;
pub use drifter::scouter::PyDrifter;
pub use profiler::scouter::DataProfiler;
pub use scouter_settings::{grpc::GrpcConfig, HttpConfig};
pub use scouter_types::dataset::{
    fingerprint_from_json_schema, inject_system_columns, json_schema_to_arrow,
};
pub use scouter_types::{
    agent::{
        utils::AssertionTasks, AgentAlertConfig, AgentAssertion, AgentAssertionTask,
        AgentEvalConfig, AgentEvalProfile, AggregationType, AssertionResult, AssertionResults,
        AssertionTask, AttributeFilterTask, ComparisonOperator, EvalResultSet, EvalScenario,
        EvalSet, EvaluationTaskType, LLMJudgeTask, MultiResponseMode, SpanFilter, SpanStatus,
        TasksFile, TokenUsage, ToolCall, TraceAssertion, TraceAssertionTask,
    },
    alert::{Alert, Alerts, CompressionType},
    create_feature_map,
    cron::*,
    custom::{CustomDriftProfile, CustomMetric, CustomMetricAlertConfig, CustomMetricDriftConfig},
    is_pydantic_basemodel,
    psi::{
        Bin, BinnedPsiFeatureMetrics, BinnedPsiMetric, PsiAlertConfig, PsiChiSquareThreshold,
        PsiDriftConfig, PsiDriftMap, PsiDriftProfile, PsiFeatureDriftProfile, PsiFixedThreshold,
        PsiNormalThreshold,
    },
    spc::{
        AlertZone, SpcAlert, SpcAlertConfig, SpcAlertRule, SpcAlertType, SpcDriftConfig,
        SpcDriftFeature, SpcDriftFeatures, SpcDriftProfile, SpcFeatureAlert, SpcFeatureAlerts,
        SpcFeatureDriftProfile,
    },
    sql::{TraceFilters, TraceListItem, TraceMetricBucket, TraceSpan},
    AgentActivityQuery, AgentBucketRow, AgentDashboardRequest, AgentDashboardResponse,
    AgentDashboardSummary, AgentEvalTaskRequest, AgentEvalTaskResponse,
    AgentEvalWorkflowPaginationResponse, AgentEvalWorkflowResult, AgentMetricBucket,
    AlertCondition, AlertDispatchType, AlertThreshold, Attribute, BinnedMetric, BinnedMetricStats,
    BinnedMetrics, ConsoleDispatchConfig, ConversationQuery, CustomMetricRecord, DataType, Doane,
    DriftAlertPaginationRequest, DriftAlertPaginationResponse, DriftProfile, DriftRequest,
    DriftType, EntityIdTagsRequest, EntityIdTagsResponse, EntityType, EqualWidthBinning,
    EvalRecord, EvalRecordPaginationRequest, EvalRecordPaginationResponse, EvalTaskResult, Feature,
    FeatureMap, Features, FreedmanDiaconis, GenAiAgentActivity, GenAiAgentActivityResponse,
    GenAiErrorBreakdownResponse, GenAiErrorCount, GenAiEvalResult, GenAiMetricsRequest,
    GenAiModelUsage, GenAiModelUsageResponse, GenAiOperationBreakdown,
    GenAiOperationBreakdownResponse, GenAiSpanFilters, GenAiSpanRecord, GenAiSpansResponse,
    GenAiTokenBucket, GenAiTokenMetricsResponse, GenAiToolActivity, GenAiToolActivityResponse,
    GenAiTraceMetricsRequest, GenAiTraceMetricsResponse, GetProfileRequest, LatencyMetrics, Manual,
    Metric, Metrics, ModelCostBreakdown, ModelPricing, ObservabilityMetrics,
    OpsGenieDispatchConfig, ProfileRequest, ProfileStatusRequest, PsiRecord, QuantileBinning,
    RecordType, RegisteredProfileResponse, Rice, RouteMetrics, Scott, ScouterResponse,
    ScouterServerError, ServerRecord, ServerRecords, SlackDispatchConfig, SpanEvent, SpanLink,
    SpcRecord, SquareRoot, Sturges, TagRecord, TagsResponse, TerrellScott, TimeInterval,
    ToolDashboardRequest, ToolDashboardResponse, ToolTimeBucket, TraceBaggageRecord,
    TraceBaggageResponse, TraceMetricsRequest, TraceMetricsResponse, TracePaginationResponse,
    TraceRecord, TraceRequest, TraceSpanRecord, TraceSpansResponse, UpdateAlertResponse,
    UpdateAlertStatus, VersionRequest, GEN_AI_AGENT_ID, GEN_AI_AGENT_NAME, GEN_AI_CONVERSATION_ID,
    GEN_AI_ERROR_TYPE, GEN_AI_OPERATION_NAME, GEN_AI_OUTPUT_TYPE, GEN_AI_PROVIDER_NAME,
    GEN_AI_REQUEST_MAX_TOKENS, GEN_AI_REQUEST_MODEL, GEN_AI_REQUEST_TEMPERATURE,
    GEN_AI_REQUEST_TOP_P, GEN_AI_RESPONSE_FINISH_REASONS, GEN_AI_RESPONSE_ID,
    GEN_AI_RESPONSE_MODEL, GEN_AI_TOOL_CALL_ID, GEN_AI_TOOL_NAME, GEN_AI_TOOL_TYPE,
    GEN_AI_USAGE_CACHE_CREATION_INPUT_TOKENS, GEN_AI_USAGE_CACHE_READ_INPUT_TOKENS,
    GEN_AI_USAGE_INPUT_TOKENS, GEN_AI_USAGE_OUTPUT_TOKENS, OPENAI_API_TYPE, OPENAI_SERVICE_TIER,
    SCOUTER_ENTITY, SCOUTER_TAG_PREFIX,
};

pub use crate::http::{PyScouterClient, ScouterClient};

pub use scouter_drift::{
    psi::PsiMonitor,
    spc::{generate_alerts, SpcDriftMap, SpcFeatureDrift, SpcMonitor},
    utils::CategoricalFeatureHelpers,
};
pub use scouter_events::error::PyEventError;
pub use scouter_events::producer::{
    kafka::KafkaConfig, mock::MockConfig, rabbitmq::RabbitMQConfig, redis::RedisConfig,
};
pub use scouter_events::queue::bus::{Flushable, TaskState};
pub use scouter_events::queue::{
    agent::EvalRecordQueue, custom::CustomMetricFeatureQueue, psi::PsiFeatureQueue,
    spc::SpcFeatureQueue, QueueBus, ScouterQueue,
};

pub use scouter_observability::Observer;
pub use scouter_profile::{
    compute_feature_correlations, CharStats, DataProfile, Distinct, FeatureProfile, Histogram,
    NumProfiler, NumericStats, Quantiles, StringProfiler, StringStats, WordStats,
};

// exposing errors
pub use scouter_drift::error::DriftError;
pub use scouter_events::error::EventError;
pub use scouter_http::error::ClientError;
pub use scouter_profile::error::DataProfileError;
pub use scouter_types::error::{ContractError, ProfileError, RecordError, TypeError, UtilError};

pub use scouter_dataframe::{extract_trace_id, infer_schema, normalize_endpoint};
pub use scouter_evaluate::{
    agent::EvalDataset,
    error::EvaluationError,
    evaluate::scenario_results::{
        EvalMetrics, ScenarioComparisonResults, ScenarioDelta, ScenarioEvalResults, ScenarioResult,
        TaskSummary,
    },
    evaluate::types::{
        AlignedEvalResult, ComparisonResults, EvalResults, EvaluationConfig, MissingTask,
        TaskComparison, WorkflowComparison,
    },
    runner::EvalRunner,
    scenario::EvalScenarios,
    tasks::agent::execute_agent_assertion_tasks,
    tasks::trace::execute_trace_assertion_tasks,
};
pub use scouter_tracing::error::TraceError;
pub use scouter_tracing::exporter::{
    processor::BatchConfig, GrpcSpanExporter, HttpSpanExporter, StdoutSpanExporter,
    TestSpanExporter,
};
pub use scouter_tracing::tracer::*;
pub use scouter_tracing::utils::{
    get_current_active_span, get_function_type, FunctionType, OtelExportConfig, OtelProtocol,
    SpanKind,
};

pub mod potato_head {
    pub use ::potato_head::prelude;
}
