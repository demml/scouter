pub mod data_utils;
pub mod drifter;
pub mod error;
pub mod http;
pub mod profiler;

pub use drifter::scouter::PyDrifter;
pub use profiler::scouter::DataProfiler;
pub use scouter_settings::HttpConfig;
pub use scouter_types::{
    alert::{Alert, Alerts, CompressionType},
    create_feature_map,
    cron::*,
    custom::{
        CustomDriftProfile, CustomMetric, CustomMetricAlertCondition, CustomMetricAlertConfig,
        CustomMetricDriftConfig,
    },
    eval::LLMEvalMetric,
    llm::{
        LLMAlertConfig, LLMDriftConfig, LLMDriftMap, LLMDriftMetric, LLMDriftProfile,
        LLMMetricAlertCondition, PaginationCursor, PaginationResponse,
    },
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
    AlertDispatchType, AlertThreshold, Attribute, BinnedMetric, BinnedMetricStats, BinnedMetrics,
    ConsoleDispatchConfig, CustomMetricRecord, DataType, Doane, DriftAlertRequest, DriftProfile,
    DriftRequest, DriftType, EntityType, EqualWidthBinning, Feature, FeatureMap, Features,
    FreedmanDiaconis, GetProfileRequest, LLMDriftRecord, LLMDriftRecordPaginationRequest,
    LLMMetricRecord, LLMRecord, LatencyMetrics, Manual, Metric, Metrics, ObservabilityMetrics,
    OpsGenieDispatchConfig, ProfileRequest, ProfileStatusRequest, PsiRecord, QuantileBinning,
    RecordType, RegisteredProfileResponse, Rice, RouteMetrics, Scott, ScouterResponse,
    ScouterServerError, ServerRecord, ServerRecords, SlackDispatchConfig, SpanEvent, SpanLink,
    SpcRecord, SquareRoot, Sturges, TagRecord, TagsResponse, TerrellScott, TimeInterval,
    TraceBaggageRecord, TraceBaggageResponse, TraceMetricsRequest, TraceMetricsResponse,
    TracePaginationResponse, TraceRecord, TraceRequest, TraceSpanRecord, TraceSpansResponse,
    UpdateAlertResponse, UpdateAlertStatus, VersionRequest, SCOUTER_TAG_PREFIX,
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
pub use scouter_events::queue::bus::TaskState;
pub use scouter_events::queue::{
    custom::CustomMetricFeatureQueue, llm::LLMRecordQueue, psi::PsiFeatureQueue,
    spc::SpcFeatureQueue, QueueBus, ScouterQueue,
};

pub use scouter_observability::Observer;
pub use scouter_profile::{
    compute_feature_correlations, CharStats, DataProfile, Distinct, FeatureProfile, Histogram,
    NumProfiler, NumericStats, Quantiles, StringProfiler, StringStats, WordStats,
};

// exposing errors
pub use error::ClientError;
pub use scouter_drift::error::DriftError;
pub use scouter_events::error::EventError;
pub use scouter_profile::error::DataProfileError;
pub use scouter_types::error::{ContractError, ProfileError, RecordError, TypeError, UtilError};

pub use scouter_evaluate::{
    error::EvaluationError,
    llm::{async_evaluate_llm, evaluate_llm, workflow_from_eval_metrics},
    types::{EvaluationConfig, LLMEvalRecord, LLMEvalResults, LLMEvalTaskResult},
};
pub use scouter_tracing::error::TraceError;
pub use scouter_tracing::exporter::{
    processor::BatchConfig, GrpcSpanExporter, HttpSpanExporter, StdoutSpanExporter,
    TestSpanExporter,
};
pub use scouter_tracing::tracer::*;
pub use scouter_tracing::utils::{
    get_function_type, ExportConfig, FunctionType, GrpcConfig, OtelHttpConfig, OtelProtocol,
    SpanKind,
};
