// This file contains the core implementation logic for tracing within Scouter.
// The goal is not to re-invent the wheel here as the opentelemetry-rust crate provides a solid implementation for tracing.
// The use case we're aiming to address is users who save models, drift profiles, llm events and want to correlate them via traces/spans.
// The only way to do that in our system is to reproduce a tracer and have it be OTEL compatible so that traces are produced
// to a collector as normal, but also produced to the Scouter backend with the relevant metadata.
// This data can then be pulled inside of OpsML's UI for trace correlation and analysis.

use crate::error::TraceError;
use crate::exporter::SpanExporterNum;
use crate::exporter::processor::BatchConfig;
use crate::exporter::scouter::ScouterSpanExporter;
use crate::resource::ScouterResourceConfig;
use crate::utils::BoxedSpan;
use crate::utils::py_obj_to_otel_keyvalue;
use crate::utils::{
    ActiveSpanInner, FunctionType, HashMapExtractor, HashMapInjector, SpanContextExt,
    capture_function_arguments, format_traceback, get_context_store, get_context_var,
    get_current_active_span, get_current_context_id, parse_span_kind, parse_status,
    set_current_span, set_function_attributes, set_function_type_attribute,
};

use chrono::{DateTime, Utc};
use opentelemetry::InstrumentationScope;
use opentelemetry::baggage::BaggageExt;
use opentelemetry::propagation::TextMapPropagator;
use opentelemetry::trace::Tracer as OTelTracer;
use opentelemetry::trace::TracerProvider;
use opentelemetry::{
    Context as OtelContext, KeyValue,
    trace::{Span, SpanContext, Status, TraceContextExt, TraceState},
};
use opentelemetry::{SpanId, TraceFlags, TraceId};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::BatchConfigBuilder as OTelBatchConfigBuilder;
use opentelemetry_sdk::trace::BatchSpanProcessor;
use opentelemetry_sdk::trace::Sampler;
use opentelemetry_sdk::trace::SdkTracer;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::trace::SpanExporter;
use potato_head::create_uuid7;
use pyo3::IntoPyObjectExt;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyTuple};
use scouter_events::queue::ScouterQueue;
use scouter_events::queue::types::TransportConfig;
use scouter_settings::grpc::GrpcConfig;
use scouter_settings::otel::{OtelProtocol as ScouterOtelProtocol, OtelSettings};

use scouter_types::{
    BAGGAGE_PREFIX, EXCEPTION_TRACEBACK, EvalRecord, SCOUTER_TAG_PREFIX, SCOUTER_TRACING_INPUT,
    SCOUTER_TRACING_LABEL, SCOUTER_TRACING_OUTPUT, SPAN_ERROR, SpanId as ScouterSpanId,
    TRACE_START_TIME_KEY, TraceId as ScouterTraceId, TraceSpanRecord, pyobject_to_otel_value,
    pyobject_to_tracing_json,
};
use scouter_types::{SCOUTER_EVAL_PROFILE_UID, SCOUTER_EVAL_RECORD_UID};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, SystemTime};
use tracing::{debug, info, instrument, warn};

/// Global static instance of the tracer provider.
static TRACER_PROVIDER_STORE: RwLock<Option<Arc<SdkTracerProvider>>> = RwLock::new(None);
static TRACE_METADATA_STORE: OnceLock<TraceMetadataStore> = OnceLock::new();

// Static ScouterQueue store for global access if needed
// This allows us to set the queue anytime get_tracer is called
static SCOUTER_QUEUE_STORE: RwLock<Option<Py<ScouterQueue>>> = RwLock::new(None);

// Re-export span capture state from scouter-types for use within this crate.
pub use scouter_types::span_capture::{CAPTURE_BUFFER_MAX, CAPTURE_BUFFERS, CAPTURING};

#[derive(Clone, Debug)]
pub struct OtlpTracingHandle {
    provider: SdkTracerProvider,
    export_timeout: Duration,
    service_name: String,
}

impl OtlpTracingHandle {
    fn new(provider: SdkTracerProvider, export_timeout: Duration, service_name: String) -> Self {
        Self {
            provider,
            export_timeout,
            service_name,
        }
    }

    pub fn tracer(&self) -> SdkTracer {
        self.provider.tracer(self.service_name.clone())
    }

    pub fn force_flush(&self) -> Result<(), TraceError> {
        self.provider.force_flush()?;
        Ok(())
    }

    pub fn shutdown(&self) -> Result<(), TraceError> {
        self.force_flush()?;
        self.provider.shutdown_with_timeout(self.export_timeout)?;
        Ok(())
    }
}

pub fn init_server_otlp_tracing(
    settings: &OtelSettings,
) -> Result<Option<OtlpTracingHandle>, TraceError> {
    if !settings.enabled {
        return Ok(None);
    }

    match settings.protocol {
        ScouterOtelProtocol::Grpc => build_server_grpc_otlp_tracing(settings).map(Some),
    }
}

fn build_server_grpc_otlp_tracing(
    settings: &OtelSettings,
) -> Result<OtlpTracingHandle, TraceError> {
    let export_timeout = Duration::from_secs(settings.export_timeout_secs);
    let resource = ScouterResourceConfig {
        service_name: Some(settings.service_name.clone()),
        ..Default::default()
    }
    .build_resource();

    let mut exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_export_config(opentelemetry_otlp::ExportConfig {
            endpoint: Some(settings.endpoint.clone()),
            protocol: opentelemetry_otlp::Protocol::Grpc,
            timeout: Some(export_timeout),
        })
        .build()?;
    exporter.set_resource(&resource);

    let batch_config = OTelBatchConfigBuilder::default()
        .with_max_export_timeout(export_timeout)
        .build();
    let span_processor = BatchSpanProcessor::builder(exporter)
        .with_batch_config(batch_config)
        .build();
    let provider = SdkTracerProvider::builder()
        .with_span_processor(span_processor)
        .with_sampler(Sampler::TraceIdRatioBased(settings.sample_ratio))
        .with_resource(resource)
        .build();

    Ok(OtlpTracingHandle::new(
        provider,
        export_timeout,
        settings.service_name.clone(),
    ))
}

/// Stable OLAP observability contract.
///
/// These names are intentionally centralized before full instrumentation lands.
/// Server, dataframe, Delta, and object-store instrumentation should use these
/// constants instead of string literals so baseline artifacts remain
/// comparable across later optimization phases.
pub mod observability_contract {
    /// HTTP route contract for the five in-scope trace endpoints.
    pub mod routes {
        pub const TRACE_PAGINATED_METHOD: &str = "POST";
        pub const TRACE_PAGINATED_PATH: &str = "{prefix}/trace/paginated";
        pub const TRACE_PAGINATED_HANDLER: &str = "paginated_traces";

        pub const TRACE_SPANS_METHOD: &str = "GET";
        pub const TRACE_SPANS_PATH: &str = "{prefix}/trace/spans";
        pub const TRACE_SPANS_HANDLER: &str = "get_trace_spans";

        pub const TRACE_METRICS_METHOD: &str = "POST";
        pub const TRACE_METRICS_PATH: &str = "{prefix}/trace/metrics";
        pub const TRACE_METRICS_HANDLER: &str = "trace_metrics";

        pub const V1_TRACE_SPANS_METHOD: &str = "GET";
        pub const V1_TRACE_SPANS_PATH: &str = "{prefix}/v1/traces/{id}/spans";
        pub const V1_TRACE_SPANS_HANDLER: &str = "get_trace_spans_by_id";

        pub const V1_TRACES_METHOD: &str = "POST";
        pub const V1_TRACES_PATH: &str = "{prefix}/v1/traces";
        pub const V1_TRACES_HANDLER: &str = "v1_otel_traces";
    }

    /// Span names used by server and analytical query instrumentation.
    pub mod span_names {
        pub const PAGINATED_TRACES_HANDLER: &str = super::routes::TRACE_PAGINATED_HANDLER;
        pub const GET_TRACE_SPANS_HANDLER: &str = super::routes::TRACE_SPANS_HANDLER;
        pub const TRACE_METRICS_HANDLER: &str = super::routes::TRACE_METRICS_HANDLER;
        pub const GET_TRACE_SPANS_BY_ID_HANDLER: &str = super::routes::V1_TRACE_SPANS_HANDLER;
        pub const V1_OTEL_TRACES_HANDLER: &str = super::routes::V1_TRACES_HANDLER;

        pub const TRACE_QUERY_PAGINATED: &str = "scouter.trace.query.paginated";
        pub const TRACE_QUERY_METRICS: &str = "scouter.trace.query.metrics";
        pub const TRACE_QUERY_SPANS: &str = "scouter.trace.query.spans";

        pub const DF_TABLE_RESOLVE: &str = "df.table.resolve";
        pub const DF_LOGICAL_BUILD: &str = "df.logical.build";
        pub const DF_PHYSICAL_PLAN: &str = "df.physical.plan";
        pub const DF_COLLECT: &str = "df.collect";
        pub const ARROW_CONVERT: &str = "arrow.convert";
        pub const TRACE_TREE_BUILD: &str = "trace.tree.build";

        pub const DELTA_TABLE_LOAD: &str = "delta.table.load";
        pub const DELTA_SNAPSHOT_REFRESH: &str = "delta.snapshot.refresh";
        pub const DELTA_CATALOG_SWAP: &str = "delta.catalog.swap";
        pub const DELTA_OPTIMIZE: &str = "delta.optimize";
        pub const UPDATE_INCREMENTAL: &str = "update_incremental";

        /// Shared object-store span name. The concrete operation is recorded in
        /// `object_store.operation` to keep span-name cardinality stable.
        pub const OBJECT_STORE_REQUEST: &str = "object_store.request";
    }

    /// Attribute keys recorded on observability spans.
    pub mod attribute_keys {
        pub const TRACE_QUERY_ENDPOINT: &str = "trace.query.endpoint";
        pub const TRACE_QUERY_KIND: &str = "trace.query.kind";
        pub const TRACE_QUERY_HAS_START_TIME: &str = "trace.query.has_start_time";
        pub const TRACE_QUERY_HAS_END_TIME: &str = "trace.query.has_end_time";
        pub const TRACE_QUERY_WINDOW_MS: &str = "trace.query.window_ms";
        pub const TRACE_QUERY_LIMIT: &str = "trace.query.limit";
        pub const TRACE_QUERY_OFFSET: &str = "trace.query.offset";
        pub const TRACE_QUERY_TRACE_ID_PRESENT: &str = "trace.query.trace_id_present";
        pub const TRACE_QUERY_UNBOUNDED: &str = "trace.query.unbounded";
        pub const TRACE_QUERY_CACHE_HIT: &str = "trace.query.cache.hit";
        pub const TRACE_QUERY_CACHE_NAME: &str = "trace.query.cache.name";
        pub const TRACE_QUERY_RESULT_ROWS: &str = "trace.query.result.rows";
        pub const TRACE_QUERY_RESULT_BYTES_ESTIMATE: &str = "trace.query.result.bytes_estimate";
        pub const TRACE_QUERY_TABLE_VERSION: &str = "trace.query.table_version";
        pub const TRACE_QUERY_STORAGE_BACKEND: &str = "trace.query.storage_backend";
        pub const TRACE_QUERY_REFRESH_ORIGIN: &str = "trace.query.refresh_origin";

        pub const OBJECT_STORE_BACKEND: &str = "object_store.backend";
        pub const OBJECT_STORE_OPERATION: &str = "object_store.operation";
        pub const OBJECT_STORE_PATH_KIND: &str = "object_store.path_kind";
        pub const OBJECT_STORE_PATH_HASH: &str = "object_store.path_hash";
        pub const OBJECT_STORE_RANGE_START: &str = "object_store.range_start";
        pub const OBJECT_STORE_RANGE_LEN: &str = "object_store.range_len";
        pub const OBJECT_STORE_CACHE_HIT: &str = "object_store.cache.hit";
        pub const OBJECT_STORE_STATUS: &str = "object_store.status";
        pub const OBJECT_STORE_ERROR_KIND: &str = "object_store.error.kind";
        pub const PARQUET_FOOTER_CANDIDATE: &str = "parquet_footer_candidate";
    }

    /// Low-cardinality attribute values used by the observability contract.
    pub mod attribute_values {
        pub const REFRESH_ORIGIN_BACKGROUND: &str = "background";
        pub const REFRESH_ORIGIN_MAINTENANCE: &str = "maintenance";
        pub const REFRESH_ORIGIN_REQUEST: &str = "request";

        pub const OBJECT_STORE_OPERATION_LIST: &str = "list";
        pub const OBJECT_STORE_OPERATION_LIST_WITH_DELIMITER: &str = "list_with_delimiter";
        pub const OBJECT_STORE_OPERATION_HEAD: &str = "head";
        pub const OBJECT_STORE_OPERATION_GET: &str = "get";
        pub const OBJECT_STORE_OPERATION_GET_RANGE: &str = "get_range";
        pub const OBJECT_STORE_OPERATION_PUT: &str = "put";
        pub const OBJECT_STORE_OPERATION_DELETE: &str = "delete";
        pub const OBJECT_STORE_OPERATION_COPY: &str = "copy";

        pub const OBJECT_STORE_PATH_KIND_DELTA_LOG: &str = "delta_log";
        pub const OBJECT_STORE_PATH_KIND_PARQUET_DATA: &str = "parquet_data";
        pub const OBJECT_STORE_PATH_KIND_CHECKPOINT: &str = "checkpoint";
        pub const OBJECT_STORE_PATH_KIND_UNKNOWN: &str = "unknown";

        pub const REFRESH_ENGINE_TRACE_SPANS: &str = "trace_spans";
        pub const REFRESH_ENGINE_TRACE_SUMMARIES: &str = "trace_summaries";
        pub const REFRESH_ENGINE_GEN_AI_SPANS: &str = "gen_ai_spans";
        pub const REFRESH_ENGINE_TRACE_DISPATCH: &str = "trace_dispatch";
        pub const REFRESH_ENGINE_BIFROST: &str = "bifrost";
        pub const REFRESH_ENGINE_EVAL_SCENARIOS: &str = "eval_scenarios";
        pub const REFRESH_ENGINE_CONTROL: &str = "control";
    }

    /// Prometheus metric names for trace OLAP observability.
    pub mod metric_names {
        pub const TRACE_QUERY_DURATION_MS: &str = "scouter_trace_query_duration_ms";
        pub const TRACE_DF_COLLECT_DURATION_MS: &str = "scouter_trace_df_collect_duration_ms";
        pub const TRACE_DF_PLAN_DURATION_MS: &str = "scouter_trace_df_plan_duration_ms";
        pub const TRACE_DELTA_REFRESH_DURATION_MS: &str = "scouter_trace_delta_refresh_duration_ms";
        pub const TRACE_OBJECT_STORE_REQUESTS_TOTAL: &str =
            "scouter_trace_object_store_requests_total";
        pub const TRACE_OBJECT_STORE_REQUEST_DURATION_MS: &str =
            "scouter_trace_object_store_request_duration_ms";
        pub const TRACE_OBJECT_STORE_BYTES_TOTAL: &str = "scouter_trace_object_store_bytes_total";
        pub const TRACE_CACHE_HITS_TOTAL: &str = "scouter_trace_cache_hits_total";
        pub const TRACE_CACHE_MISSES_TOTAL: &str = "scouter_trace_cache_misses_total";
        pub const TRACE_UNBOUNDED_LOOKUP_TOTAL: &str = "scouter_trace_unbounded_lookup_total";
        pub const REFRESH_ON_REQUEST_PATH_TOTAL: &str = "scouter_refresh_on_request_path_total";
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum MetricKind {
        Counter,
        Histogram,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct MetricContract {
        pub name: &'static str,
        pub kind: MetricKind,
        pub description: &'static str,
        pub labels: &'static [&'static str],
    }

    pub const SPAN_NAMES: &[&str] = &[
        span_names::PAGINATED_TRACES_HANDLER,
        span_names::GET_TRACE_SPANS_HANDLER,
        span_names::TRACE_METRICS_HANDLER,
        span_names::GET_TRACE_SPANS_BY_ID_HANDLER,
        span_names::V1_OTEL_TRACES_HANDLER,
        span_names::TRACE_QUERY_PAGINATED,
        span_names::TRACE_QUERY_METRICS,
        span_names::TRACE_QUERY_SPANS,
        span_names::DF_TABLE_RESOLVE,
        span_names::DF_LOGICAL_BUILD,
        span_names::DF_PHYSICAL_PLAN,
        span_names::DF_COLLECT,
        span_names::ARROW_CONVERT,
        span_names::TRACE_TREE_BUILD,
        span_names::DELTA_TABLE_LOAD,
        span_names::DELTA_SNAPSHOT_REFRESH,
        span_names::DELTA_CATALOG_SWAP,
        span_names::DELTA_OPTIMIZE,
        span_names::UPDATE_INCREMENTAL,
        span_names::OBJECT_STORE_REQUEST,
    ];

    pub const TRACE_QUERY_ATTRIBUTE_KEYS: &[&str] = &[
        attribute_keys::TRACE_QUERY_ENDPOINT,
        attribute_keys::TRACE_QUERY_KIND,
        attribute_keys::TRACE_QUERY_HAS_START_TIME,
        attribute_keys::TRACE_QUERY_HAS_END_TIME,
        attribute_keys::TRACE_QUERY_WINDOW_MS,
        attribute_keys::TRACE_QUERY_LIMIT,
        attribute_keys::TRACE_QUERY_OFFSET,
        attribute_keys::TRACE_QUERY_TRACE_ID_PRESENT,
        attribute_keys::TRACE_QUERY_UNBOUNDED,
        attribute_keys::TRACE_QUERY_CACHE_HIT,
        attribute_keys::TRACE_QUERY_CACHE_NAME,
        attribute_keys::TRACE_QUERY_RESULT_ROWS,
        attribute_keys::TRACE_QUERY_RESULT_BYTES_ESTIMATE,
        attribute_keys::TRACE_QUERY_TABLE_VERSION,
        attribute_keys::TRACE_QUERY_STORAGE_BACKEND,
        attribute_keys::TRACE_QUERY_REFRESH_ORIGIN,
    ];

    pub const OBJECT_STORE_ATTRIBUTE_KEYS: &[&str] = &[
        attribute_keys::OBJECT_STORE_BACKEND,
        attribute_keys::OBJECT_STORE_OPERATION,
        attribute_keys::OBJECT_STORE_PATH_KIND,
        attribute_keys::OBJECT_STORE_PATH_HASH,
        attribute_keys::OBJECT_STORE_RANGE_START,
        attribute_keys::OBJECT_STORE_RANGE_LEN,
        attribute_keys::OBJECT_STORE_CACHE_HIT,
        attribute_keys::OBJECT_STORE_STATUS,
        attribute_keys::OBJECT_STORE_ERROR_KIND,
        attribute_keys::PARQUET_FOOTER_CANDIDATE,
    ];

    pub const METRIC_CONTRACTS: &[MetricContract] = &[
        MetricContract {
            name: metric_names::TRACE_QUERY_DURATION_MS,
            kind: MetricKind::Histogram,
            description: "End-to-end duration for trace query handlers.",
            labels: &["endpoint", "kind", "unbounded"],
        },
        MetricContract {
            name: metric_names::TRACE_DF_COLLECT_DURATION_MS,
            kind: MetricKind::Histogram,
            description: "Duration spent in DataFusion collect() for trace queries.",
            labels: &["endpoint", "table"],
        },
        MetricContract {
            name: metric_names::TRACE_DF_PLAN_DURATION_MS,
            kind: MetricKind::Histogram,
            description: "Duration spent building DataFusion logical or physical plans.",
            labels: &["endpoint", "phase"],
        },
        MetricContract {
            name: metric_names::TRACE_DELTA_REFRESH_DURATION_MS,
            kind: MetricKind::Histogram,
            description: "Duration spent refreshing Delta snapshots for trace tables.",
            labels: &["engine", "origin"],
        },
        MetricContract {
            name: metric_names::TRACE_OBJECT_STORE_REQUESTS_TOTAL,
            kind: MetricKind::Counter,
            description: "Object-store requests issued by trace analytical paths.",
            labels: &["backend", "operation", "path_kind", "status"],
        },
        MetricContract {
            name: metric_names::TRACE_OBJECT_STORE_REQUEST_DURATION_MS,
            kind: MetricKind::Histogram,
            description: "Object-store request duration for trace analytical paths.",
            labels: &["backend", "operation", "path_kind", "status"],
        },
        MetricContract {
            name: metric_names::TRACE_OBJECT_STORE_BYTES_TOTAL,
            kind: MetricKind::Counter,
            description: "Object-store bytes read or written by trace analytical paths.",
            labels: &["backend", "operation", "path_kind"],
        },
        MetricContract {
            name: metric_names::TRACE_CACHE_HITS_TOTAL,
            kind: MetricKind::Counter,
            description: "Cache hits observed by trace analytical paths.",
            labels: &["cache_name"],
        },
        MetricContract {
            name: metric_names::TRACE_CACHE_MISSES_TOTAL,
            kind: MetricKind::Counter,
            description: "Cache misses observed by trace analytical paths.",
            labels: &["cache_name"],
        },
        MetricContract {
            name: metric_names::TRACE_UNBOUNDED_LOOKUP_TOTAL,
            kind: MetricKind::Counter,
            description: "Trace lookups issued without explicit time bounds.",
            labels: &["endpoint", "kind"],
        },
        MetricContract {
            name: metric_names::REFRESH_ON_REQUEST_PATH_TOTAL,
            kind: MetricKind::Counter,
            description: "Delta refreshes observed on synchronous request paths.",
            labels: &["engine"],
        },
    ];

    pub const OBJECT_STORE_OPERATIONS: &[&str] = &[
        attribute_values::OBJECT_STORE_OPERATION_LIST,
        attribute_values::OBJECT_STORE_OPERATION_LIST_WITH_DELIMITER,
        attribute_values::OBJECT_STORE_OPERATION_HEAD,
        attribute_values::OBJECT_STORE_OPERATION_GET,
        attribute_values::OBJECT_STORE_OPERATION_GET_RANGE,
        attribute_values::OBJECT_STORE_OPERATION_PUT,
        attribute_values::OBJECT_STORE_OPERATION_DELETE,
        attribute_values::OBJECT_STORE_OPERATION_COPY,
    ];

    pub const REFRESH_ON_REQUEST_ENGINES: &[&str] = &[
        attribute_values::REFRESH_ENGINE_TRACE_SPANS,
        attribute_values::REFRESH_ENGINE_TRACE_SUMMARIES,
        attribute_values::REFRESH_ENGINE_GEN_AI_SPANS,
        attribute_values::REFRESH_ENGINE_TRACE_DISPATCH,
        attribute_values::REFRESH_ENGINE_BIFROST,
        attribute_values::REFRESH_ENGINE_EVAL_SCENARIOS,
        attribute_values::REFRESH_ENGINE_CONTROL,
    ];
}

fn get_tracer_provider() -> Result<Option<Arc<SdkTracerProvider>>, TraceError> {
    TRACER_PROVIDER_STORE
        .read()
        .map(|guard| guard.clone())
        .map_err(|e| TraceError::PoisonError(e.to_string()))
}

#[derive(Clone)]
struct TraceMetadata {
    start_time: DateTime<Utc>,
    span_count: u32,
}

#[derive(Clone)]
struct TraceMetadataStore {
    inner: Arc<RwLock<HashMap<String, TraceMetadata>>>,
}

impl TraceMetadataStore {
    fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn set_trace_start(
        &self,
        trace_id: String,
        start_time: DateTime<Utc>,
    ) -> Result<(), TraceError> {
        self.inner
            .write()
            .map_err(|e| TraceError::PoisonError(e.to_string()))?
            .insert(
                trace_id.clone(),
                TraceMetadata {
                    start_time,
                    span_count: 0,
                },
            );
        Ok(())
    }

    fn get_trace_metadata(&self, trace_id: &str) -> Result<Option<TraceMetadata>, TraceError> {
        Ok(self
            .inner
            .read()
            .map_err(|e| TraceError::PoisonError(e.to_string()))?
            .get(trace_id)
            .cloned())
    }

    fn increment_span_count(&self, trace_id: &str) -> Result<(), TraceError> {
        let mut map = self
            .inner
            .write()
            .map_err(|e| TraceError::PoisonError(e.to_string()))?;
        if let Some(m) = map.get_mut(trace_id) {
            m.span_count += 1;
        }
        Ok(())
    }

    /// Decrements the span count for the given trace ID. If the span count reaches zero, the trace metadata is removed.
    fn decrement_span_count(&self, trace_id: &str) -> Result<(), TraceError> {
        let mut map = self
            .inner
            .write()
            .map_err(|e| TraceError::PoisonError(e.to_string()))?;
        match map.get_mut(trace_id) {
            Some(m) if m.span_count > 1 => {
                m.span_count -= 1;
            }
            Some(_) => {
                map.remove(trace_id);
            }
            None => {}
        }
        Ok(())
    }

    fn clear_all(&self) -> Result<(), TraceError> {
        self.inner
            .write()
            .map_err(|e| TraceError::PoisonError(e.to_string()))?
            .clear();
        Ok(())
    }
}

fn get_trace_metadata_store() -> &'static TraceMetadataStore {
    TRACE_METADATA_STORE.get_or_init(TraceMetadataStore::new)
}

/// Configure the process-wide tracer provider exactly once.
///
/// Builds an OTel Resource (honoring `OTEL_SERVICE_NAME` /
/// `OTEL_RESOURCE_ATTRIBUTES` env vars per spec), constructs the underlying
/// `SdkTracerProvider`, and stores it in `TRACER_PROVIDER_STORE`.
///
/// If a provider already exists, logs a warning and returns Ok without
/// rebuilding (matches OTel SDK `SetTracerProvider` semantics — second call
/// is a no-op).
///
/// # Arguments
/// * `resource_config` - process-wide Resource (service.name, etc.)
/// * `transport_config` - Optional transport for the Scouter exporter
/// * `exporter` - Optional secondary OTLP exporter
/// * `batch_config` - Optional batch span processor configuration
/// * `sample_ratio` - Optional sampling ratio in [0.0, 1.0]
#[pyfunction]
#[pyo3(signature = (
    resource_config = None,
    transport_config=None,
    exporter=None,
    batch_config=None,
    sample_ratio=None,
))]
#[instrument(skip_all)]
pub fn configure_tracing(
    py: Python,
    resource_config: Option<Py<crate::resource::ScouterResourceConfig>>,
    transport_config: Option<&Bound<'_, PyAny>>,
    exporter: Option<&Bound<'_, PyAny>>,
    batch_config: Option<Py<BatchConfig>>,
    sample_ratio: Option<f64>,
) -> Result<(), TraceError> {
    let provider_exists = TRACER_PROVIDER_STORE
        .read()
        .map_err(|e| TraceError::PoisonError(e.to_string()))?
        .is_some();

    if provider_exists {
        tracing::warn!(
            "configure_tracing called more than once; subsequent calls are no-ops. \
             Existing provider retained."
        );
        return Ok(());
    }

    let resource = match resource_config {
        Some(cfg) => cfg
            .extract::<crate::resource::ScouterResourceConfig>(py)?
            .build_resource(),
        None => crate::resource::ScouterResourceConfig::default().build_resource(),
    };

    let transport_config = match transport_config {
        Some(config) => TransportConfig::from_py_config(config)?,
        None => {
            if std::env::var("SCOUTER_OFFLINE").as_deref() == Ok("1") {
                TransportConfig::offline_mock()
            } else {
                TransportConfig::Grpc(GrpcConfig::default())
            }
        }
    };

    let clamped_sample_ratio = match sample_ratio {
        Some(ratio) if (0.0..=1.0).contains(&ratio) => Some(ratio),
        Some(ratio) => {
            info!(
                "Sample ratio {} is out of bounds [0.0, 1.0]. Clamping to valid range.",
                ratio
            );
            Some(ratio.clamp(0.0, 1.0))
        }
        None => None,
    };

    let batch_config = if let Some(bc) = batch_config {
        Some(bc.extract::<BatchConfig>(py)?)
    } else {
        None
    };

    let scouter_export = ScouterSpanExporter::new(transport_config, &resource)?;

    let mut span_exporter = if let Some(exporter) = exporter {
        SpanExporterNum::from_pyobject(exporter)
            .map_err(|_| TraceError::UnsupportedSpanExporterType)?
    } else {
        SpanExporterNum::default()
    };
    span_exporter.set_sample_ratio(clamped_sample_ratio);

    let provider = span_exporter
        .build_provider(resource, scouter_export, batch_config)
        .map_err(|e| TraceError::InitializationError(e.to_string()))?;

    let mut store_guard = TRACER_PROVIDER_STORE
        .write()
        .map_err(|e| TraceError::PoisonError(e.to_string()))?;

    if store_guard.is_none() {
        *store_guard = Some(Arc::new(provider));
    }
    Ok(())
}

/// Get a tracer scoped to an instrumenting library/module.
///
/// `scope_name` and `scope_version` populate the OTel `InstrumentationScope`.
/// They are independent of the process-wide `Resource.service.name`.
///
/// If `configure_tracing` has not been called, a default Resource is built
/// from environment variables (matches OTel SDK "always usable provider"
/// semantics).
#[pyfunction]
#[pyo3(signature = (
    scope_name,
    scope_version = None,
    schema_url = None,
    scope_attributes = None,
    default_attributes = None,
    scouter_queue = None,
))]
#[instrument(skip_all)]
#[allow(clippy::too_many_arguments)]
pub fn get_tracer(
    py: Python,
    scope_name: String,
    scope_version: Option<String>,
    schema_url: Option<String>,
    scope_attributes: Option<Bound<'_, PyAny>>,
    default_attributes: Option<Bound<'_, PyAny>>,
    scouter_queue: Option<Py<ScouterQueue>>,
) -> Result<BaseTracer, TraceError> {
    let provider_exists = TRACER_PROVIDER_STORE
        .read()
        .map_err(|e| TraceError::PoisonError(e.to_string()))?
        .is_some();

    if !provider_exists {
        // Lazy default: build provider from env-only Resource.
        configure_tracing(py, None, None, None, None, None)?;
    }

    BaseTracer::new(
        py,
        scope_name,
        scope_version,
        schema_url,
        scope_attributes,
        default_attributes,
        scouter_queue,
    )
}

fn reset_current_context(py: Python, token: &Py<PyAny>) -> PyResult<()> {
    let context_var = get_context_var(py)?;
    match context_var.bind(py).call_method1("reset", (token,)) {
        Ok(_) => Ok(()),
        Err(e) if e.is_instance_of::<pyo3::exceptions::PyValueError>(py) => Ok(()),
        Err(e) => Err(e),
    }
}

/// ActiveSpan where all the magic happens
/// The active Span attempts to maintain compatibility with the OpenTelemetry Span API
#[pyclass(skip_from_py_object)]
pub struct ActiveSpan {
    inner: Arc<RwLock<ActiveSpanInner>>,
}

#[pymethods]
impl ActiveSpan {
    #[getter]
    fn trace_id(&self) -> Result<String, TraceError> {
        self.with_inner(|inner| inner.span.span_context().trace_id().to_string())
    }

    #[getter]
    fn span_id(&self) -> Result<String, TraceError> {
        self.with_inner(|inner| inner.span.span_context().span_id().to_string())
    }

    #[getter]
    fn context_id(&self) -> Result<String, TraceError> {
        self.with_inner(|inner| inner.context_id.clone())
    }

    #[getter]
    fn parent_context_id(&self) -> Result<Option<String>, TraceError> {
        self.with_inner(|inner| inner.parent_context_id.clone())
    }

    /// Set the input attribute on the span
    /// # Arguments
    /// * `input` - The input value (any Python object, but is often a dict)
    /// * `max_length` - Maximum length of the serialized input (default: 1000)
    #[pyo3(signature = (input, max_length=1000))]
    #[instrument(skip_all)]
    fn set_input(&self, input: &Bound<'_, PyAny>, max_length: usize) -> Result<(), TraceError> {
        let value = pyobject_to_tracing_json(input, &max_length)?;
        self.with_inner_mut(|inner| {
            inner.span.set_attribute(KeyValue::new(
                SCOUTER_TRACING_INPUT,
                serde_json::to_string(&value).unwrap(),
            ))
        })
    }

    #[pyo3(signature = (output, max_length=1000))]
    #[instrument(skip_all)]
    fn set_output(&self, output: &Bound<'_, PyAny>, max_length: usize) -> Result<(), TraceError> {
        let value = pyobject_to_tracing_json(output, &max_length)?;
        self.with_inner_mut(|inner| {
            inner.span.set_attribute(KeyValue::new(
                SCOUTER_TRACING_OUTPUT,
                serde_json::to_string(&value).unwrap(),
            ))
        })
    }

    /// Set an attribute on the span
    /// # Arguments
    /// * `key` - The attribute key
    /// * `value` - The attribute value
    pub fn set_attribute(&self, key: String, value: Bound<'_, PyAny>) -> Result<(), TraceError> {
        let value = pyobject_to_otel_value(&value)?;
        self.with_inner_mut(|inner| inner.span.set_attribute(KeyValue::new(key, value)))
    }

    /// Set a tag on the span (alias for set_attribute)
    /// Tags are slightly different in that they are often used for indexing and searching
    /// On export, tags are exported and stored in a separate table in Scouter for easier
    /// searching/filtering.
    pub fn set_tag(&self, key: String, value: Bound<'_, PyAny>) -> Result<(), TraceError> {
        // backend searches for tags with the prefix scouter.tag.*
        // this prefix will be stripped before storage
        let tag_key = format!("{}.{}", SCOUTER_TAG_PREFIX, key);
        self.set_attribute(tag_key, value)
    }

    /// Add an event to the span
    /// # Arguments
    /// * `name` - The event name
    /// * `attributes` - The event attributes as a dictionary or pydantic BaseModel
    /// * `timestamp` - Optional timestamp in nanoseconds since Unix epoch (OTel compatible)
    #[pyo3(signature = (name, attributes=None, timestamp=None))]
    fn add_event(
        &self,
        py: Python,
        name: String,
        attributes: Option<Bound<'_, PyAny>>,
        timestamp: Option<i64>,
    ) -> Result<(), TraceError> {
        let pairs: Vec<KeyValue> = py_obj_to_otel_keyvalue(py, attributes)?;
        self.with_inner_mut(|inner| {
            if let Some(ts) = timestamp {
                let system_time =
                    SystemTime::UNIX_EPOCH + std::time::Duration::from_nanos(ts as u64);
                inner
                    .span
                    .add_event_with_timestamp(name, system_time, pairs);
            } else {
                inner.span.add_event(name, pairs);
            }
        })
    }

    /// Build a trace-anchored EvalRecord from this span's context and insert it
    /// into the tracer's attached queue.
    ///
    /// This is the Path A API for agentic eval records. The trace anchor is
    /// taken from the live span handle, not ambient OTel context, baggage, or a
    /// caller-provided EvalRecord. If the trace sampled flag is false, no record
    /// is inserted because no trace commit event will arrive to release the row
    /// from `awaiting_trace`.
    ///
    /// # Arguments
    /// * `profile_uid` - The eval profile UID this record is associated with.
    /// * `context` - The record context. Either a `dict` or a pydantic BaseModel.
    /// * `record_id` - Optional caller-defined scenario, turn, step, or callback ID.
    /// * `session_id` - Optional session/thread/conversation ID.
    /// * `media` - Optional per-record multimodal evidence.
    /// * `tags` - Optional `key=value` tags stored with the eval record.
    ///
    /// # Errors
    /// Returns `RuntimeError` if the tracer this span belongs to has no queue
    /// attached, if queue lookup by profile UID fails, or if the record cannot
    /// be constructed from the supplied context/media.
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (profile_uid, context, *, record_id=None, session_id=None, media=None, tags=None))]
    fn attach_eval(
        &self,
        py: Python<'_>,
        profile_uid: String,
        context: Bound<'_, PyAny>,
        record_id: Option<String>,
        session_id: Option<String>,
        media: Option<Vec<Bound<'_, PyAny>>>,
        tags: Option<Vec<String>>,
    ) -> Result<(), TraceError> {
        debug!(
            "Attaching eval record (profile_uid={}) to span {}",
            profile_uid,
            self.context_id()?
        );

        let trace_sampled =
            self.with_inner(|inner| inner.span.span_context().trace_flags().is_sampled())?;
        if !trace_sampled {
            debug!(
                "Skipping attach_eval for profile_uid={} because the trace is not sampled",
                profile_uid
            );
            return Ok(());
        }

        let (trace_id, span_id) = self.with_inner(|inner| {
            let span_context = inner.span.span_context();
            (
                ScouterTraceId::from_bytes(span_context.trace_id().to_bytes()),
                ScouterSpanId::from_bytes(span_context.span_id().to_bytes()),
            )
        })?;

        let record = EvalRecord::new_trace_attached(
            py,
            Some(context),
            record_id,
            session_id,
            media,
            profile_uid.clone(),
            tags,
            trace_id,
            span_id,
        )?;

        if record.trace_id.is_none() || record.span_id.is_none() {
            return Err(TraceError::SpanError(
                "attach_eval could not build a valid trace anchor from the active span".to_string(),
            ));
        }

        let record_uid = record.uid.clone();

        self.with_inner_mut(|inner| -> Result<(), TraceError> {
            let queue = inner.queue.as_ref().ok_or_else(|| {
                TraceError::QueueNotConfigured(
                    "attach_eval called on a span whose tracer has no queue. Pass `scouter_queue=...` to `get_tracer(...)` or set it via `tracer.set_scouter_queue(queue)`."
                        .to_string(),
                )
            })?;

            let bound_queue = queue
                .bind(py)
                .call_method1("get_by_entity_uid", (&profile_uid,))?;
            let py_record = Py::new(py, record)?;
            bound_queue.call_method1("insert", (py_record.bind(py),))?;

            inner
                .span
                .set_attribute(KeyValue::new(SCOUTER_EVAL_RECORD_UID, record_uid));
            inner
                .span
                .set_attribute(KeyValue::new(SCOUTER_EVAL_PROFILE_UID, profile_uid));

            Ok(())
        })?
    }

    /// Set the status of the span
    /// # Arguments
    /// * `status` - The status string ("ok", "error", or "unset")
    /// * `description` - Optional description for the status (typically used with error)
    #[pyo3(signature = (status, description=None))]
    fn set_status(
        &self,
        status: &Bound<'_, PyAny>,
        description: Option<String>,
    ) -> Result<(), TraceError> {
        let otel_status = parse_status(status, description);
        self.with_inner_mut(|inner| inner.span.set_status(otel_status))
    }

    /// Sync context manager enter
    #[instrument(skip_all)]
    fn __enter__<'py>(slf: PyRef<'py, Self>) -> PyResult<PyRef<'py, Self>> {
        debug!("Entering span context: {}", slf.context_id()?);
        Ok(slf)
    }

    /// Sync context manager exit
    #[pyo3(signature = (exc_type=None, exc_val=None, exc_tb=None))]
    #[instrument(skip_all)]
    fn __exit__(
        &mut self,
        py: Python<'_>,
        exc_type: Option<Py<PyAny>>,
        exc_val: Option<Py<PyAny>>,
        exc_tb: Option<Py<PyAny>>,
    ) -> Result<bool, TraceError> {
        debug!("Exiting span context: {}", self.context_id()?);
        {
            let mut inner = self
                .inner
                .write()
                .map_err(|e| TraceError::PoisonError(e.to_string()))?;

            if !inner.cleanup_complete {
                // Handle exceptions and end span
                if let Some(exc_type) = exc_type {
                    inner.span.set_status(Status::error("Exception occurred"));
                    let mut error_attributes = vec![];

                    error_attributes.push(KeyValue::new("exception.type", exc_type.to_string()));

                    if let Some(exc_val) = exc_val {
                        error_attributes
                            .push(KeyValue::new("exception.value", exc_val.to_string()));
                    }

                    if let Some(exc_tb) = exc_tb {
                        let tb = format_traceback(py, &exc_tb)?;
                        error_attributes.push(KeyValue::new(EXCEPTION_TRACEBACK, tb));
                    }

                    inner.span.add_event(SPAN_ERROR, error_attributes);
                }
                // else set status to ok
                else {
                    inner.span.set_status(Status::Ok);
                }
            }
        }

        self.complete_span(py, None)?;
        Ok(false)
    }

    /// Async context manager enter
    fn __aenter__<'py>(slf: PyRef<'py, Self>, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let slf_py: Py<PyAny> = slf.into_py_any(py)?;

        // We need to return a Future that resolves to slf_py (__aenter__ is expected to return an awaitable)
        pyo3_async_runtimes::tokio::future_into_py(py, async move { Ok(slf_py) })
    }

    /// Async context manager exit
    #[pyo3(signature = (exc_type=None, exc_val=None, exc_tb=None))]
    fn __aexit__<'py>(
        &mut self,
        py: Python<'py>,
        exc_type: Option<Py<PyAny>>,
        exc_val: Option<Py<PyAny>>,
        exc_tb: Option<Py<PyAny>>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let result = self.__exit__(py, exc_type, exc_val, exc_tb)?;
        let py_result = result.into_py_any(py)?;

        // We need to return a Future that resolves to py_result (__aexit__ is expected to return an awaitable)
        pyo3_async_runtimes::tokio::future_into_py(py, async move { Ok(py_result) })
    }

    fn end(&self, py: Python<'_>, end_time: Option<i64>) -> Result<(), TraceError> {
        self.complete_span(py, end_time)
    }

    #[pyo3(name = "_end_with_cleanup", signature = (end_time=None))]
    fn end_with_cleanup(&self, py: Python<'_>, end_time: Option<i64>) -> Result<(), TraceError> {
        self.complete_span(py, end_time)
    }

    /// Returns an OTel-compatible SpanContext for this span.
    /// The returned object is a `opentelemetry.trace.SpanContext` namedtuple with integer
    /// trace_id and span_id, suitable for interop with other OTel instrumentation.
    fn get_span_context<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyAny>, TraceError> {
        let span_ctx = self.with_inner(|inner| inner.span.span_context().clone())?;

        let trace_id_int = u128::from_be_bytes(span_ctx.trace_id().to_bytes());
        let span_id_int = u64::from_be_bytes(span_ctx.span_id().to_bytes());
        let is_remote = span_ctx.is_remote();
        let trace_flags_u8 = span_ctx.trace_flags().to_u8();
        let trace_state_header = span_ctx.trace_state().header();

        let otel_trace = py.import("opentelemetry.trace")?;
        let trace_flags_cls = otel_trace.getattr("TraceFlags")?;
        let trace_state_cls = otel_trace.getattr("TraceState")?;
        let span_ctx_cls = otel_trace.getattr("SpanContext")?;

        let py_trace_flags = trace_flags_cls.call1((trace_flags_u8,))?;
        let trace_state_entries = PyList::empty(py);
        if !trace_state_header.is_empty() {
            for member in trace_state_header.split(',') {
                if let Some((key, value)) = member.split_once('=') {
                    trace_state_entries.append((key, value))?;
                }
            }
        }
        let py_trace_state = trace_state_cls.call1((trace_state_entries,))?;

        let ctx = span_ctx_cls.call1((
            trace_id_int,
            span_id_int,
            is_remote,
            py_trace_flags,
            py_trace_state,
        ))?;

        Ok(ctx)
    }

    /// Sets multiple attributes on the span from a dict (OTel-compatible bulk setter).
    fn set_attributes(&self, attributes: &Bound<'_, PyAny>) -> Result<(), TraceError> {
        if let Ok(dict) = attributes.cast::<pyo3::types::PyDict>() {
            for (key, value) in dict.iter() {
                let key_str = key.extract::<String>()?;
                self.set_attribute(key_str, value)?
            }
        }
        Ok(())
    }

    /// Updates the span name (OTel-compatible).
    fn update_name(&self, name: String) -> Result<(), TraceError> {
        self.with_inner_mut(|inner| inner.span.update_name(Cow::Owned(name)))
    }

    /// Returns true if this span is active and recording (OTel-compatible).
    fn is_recording(&self) -> Result<bool, TraceError> {
        self.with_inner(|inner| inner.span.is_recording())
    }

    /// Records an exception as a span event following OTel semantic conventions.
    /// # Arguments
    /// * `exception` - The Python exception to record
    /// * `attributes` - Optional extra attributes dict
    /// * `timestamp` - Optional timestamp in nanoseconds since Unix epoch
    /// * `escaped` - Whether the exception escaped the span's scope
    #[pyo3(signature = (exception, attributes=None, timestamp=None, escaped=false))]
    fn record_exception(
        &self,
        py: Python<'_>,
        exception: &Bound<'_, PyAny>,
        attributes: Option<Bound<'_, PyAny>>,
        timestamp: Option<i64>,
        escaped: bool,
    ) -> Result<(), TraceError> {
        let exc_type = exception.get_type();
        let module = exc_type
            .getattr("__module__")
            .ok()
            .and_then(|m| m.extract::<String>().ok());
        let qualname = exc_type
            .getattr("__qualname__")
            .ok()
            .and_then(|q| q.extract::<String>().ok());
        let type_name = match (module, qualname) {
            (Some(m), Some(q)) if m != "builtins" => format!("{}.{}", m, q),
            (_, Some(q)) => q,
            _ => "UnknownException".to_string(),
        };

        let message = exception.str()?.to_string();

        let mut event_attrs = vec![
            KeyValue::new("exception.type", type_name),
            KeyValue::new("exception.message", message),
            KeyValue::new("exception.escaped", escaped.to_string()),
        ];

        if let Ok(tb_py) = exception.getattr("__traceback__")
            && !tb_py.is_none()
        {
            let tb_unbound: Py<PyAny> = tb_py.unbind();
            if let Ok(stacktrace) = format_traceback(py, &tb_unbound) {
                event_attrs.push(KeyValue::new("exception.stacktrace", stacktrace));
            }
        }

        let extra_attrs = py_obj_to_otel_keyvalue(py, attributes)?;
        event_attrs.extend(extra_attrs);

        self.with_inner_mut(|inner| {
            if let Some(ts) = timestamp {
                let system_time =
                    SystemTime::UNIX_EPOCH + std::time::Duration::from_nanos(ts as u64);
                inner
                    .span
                    .add_event_with_timestamp("exception", system_time, event_attrs);
            } else {
                inner.span.add_event("exception", event_attrs);
            }
        })
    }

    /// Adds a link to another span (OTel-compatible no-op placeholder).
    /// Full cross-process link support is not yet implemented; this method is provided
    /// so that code written against the OTel Span ABC compiles without errors.
    #[pyo3(signature = (context, attributes=None))]
    fn add_link(
        &self,
        py: Python<'_>,
        context: &Bound<'_, PyAny>,
        attributes: Option<Bound<'_, PyAny>>,
    ) -> Result<(), TraceError> {
        let span_context = SpanContext::from_py_span_context(context)?;
        let attributes = py_obj_to_otel_keyvalue(py, attributes)?;

        self.with_inner_mut(|inner| {
            inner.span.add_link(span_context, attributes);
        })
    }
}

impl ActiveSpan {
    fn complete_span(&self, py: Python<'_>, end_time: Option<i64>) -> Result<(), TraceError> {
        let (context_id, trace_id, context_token, should_cleanup) = {
            let mut inner = self
                .inner
                .write()
                .map_err(|e| TraceError::PoisonError(e.to_string()))?;

            if inner.cleanup_complete {
                (None, None, inner.context_token.take(), false)
            } else {
                if let Some(ts) = end_time {
                    let system_time =
                        SystemTime::UNIX_EPOCH + std::time::Duration::from_nanos(ts as u64);
                    inner.span.end_with_timestamp(system_time);
                } else {
                    inner.span.end();
                }

                let context_token = inner.context_token.take();

                let context_id = inner.context_id.clone();
                let trace_id = inner.span.span_context().trace_id().to_string();
                inner.queue.take();
                inner.cleanup_complete = true;

                (Some(context_id), Some(trace_id), context_token, true)
            }
        };

        if let Some(token) = context_token {
            reset_current_context(py, &token)?;
        }

        if should_cleanup {
            if let Some(trace_id) = trace_id {
                get_trace_metadata_store().decrement_span_count(&trace_id)?;
            }
            if let Some(context_id) = context_id {
                get_context_store().remove(&context_id)?;
            }
        }

        Ok(())
    }

    pub fn set_attribute_static(
        &mut self,
        key: &'static str,
        value: String,
    ) -> Result<(), TraceError> {
        self.inner
            .write()
            .map_err(|e| TraceError::PoisonError(e.to_string()))?
            .span
            .set_attribute(KeyValue::new(key, value));
        Ok(())
    }

    fn with_inner_mut<F, R>(&self, f: F) -> Result<R, TraceError>
    where
        F: FnOnce(&mut ActiveSpanInner) -> R,
    {
        let mut inner = self
            .inner
            .write()
            .map_err(|e| TraceError::PoisonError(e.to_string()))?;
        Ok(f(&mut inner))
    }

    fn with_inner<F, R>(&self, f: F) -> Result<R, TraceError>
    where
        F: FnOnce(&ActiveSpanInner) -> R,
    {
        let inner = self
            .inner
            .read()
            .map_err(|e| TraceError::PoisonError(e.to_string()))?;
        Ok(f(&inner))
    }
}

/// The main Tracer class
#[pyclass(subclass)]
pub struct BaseTracer {
    tracer: SdkTracer,
    queue: Option<Py<ScouterQueue>>,
    default_attributes: Vec<KeyValue>,
}

impl BaseTracer {
    fn set_start_time(&self, span: &mut BoxedSpan) {
        let trace_id = span.span_context().trace_id().to_string();
        let trace_metadata_store = get_trace_metadata_store();

        let start_time = match trace_metadata_store.get_trace_metadata(&trace_id) {
            Ok(Some(metadata)) => {
                // Use existing trace start time
                metadata.start_time
            }
            Ok(None) => {
                // Create new trace metadata with current time
                let current_time = Utc::now();
                if let Err(e) = trace_metadata_store.set_trace_start(trace_id, current_time) {
                    tracing::warn!("Failed to set trace start time: {}", e);
                }
                current_time
            }
            Err(e) => {
                tracing::warn!("Failed to get trace metadata: {}", e);
                Utc::now()
            }
        };

        span.set_attribute(KeyValue::new(TRACE_START_TIME_KEY, start_time.to_rfc3339()));
    }

    fn increment_span_count(&self, trace_id: &str) -> Result<(), TraceError> {
        let trace_metadata_store = get_trace_metadata_store();
        trace_metadata_store.increment_span_count(trace_id)
    }

    fn setup_trace_metadata(&self, span: &mut BoxedSpan) -> Result<(), TraceError> {
        let trace_id = span.span_context().trace_id().to_string();
        self.set_start_time(span);
        self.increment_span_count(&trace_id)?;
        Ok(())
    }

    fn create_baggage_items(
        baggage: &[HashMap<String, String>],
        tags: &[HashMap<String, String>],
    ) -> Vec<KeyValue> {
        let mut keyval_baggage: Vec<KeyValue> = baggage
            .iter()
            .flat_map(|baggage_map| {
                baggage_map
                    .iter()
                    .map(|(k, v)| KeyValue::new(k.clone(), v.clone()))
                    .collect::<Vec<KeyValue>>()
            })
            .collect();

        // add tags to baggage
        tags.iter().for_each(|tag_map| {
            tag_map.iter().for_each(|(k, v)| {
                keyval_baggage.push(KeyValue::new(
                    format!("{}.{}.{}", BAGGAGE_PREFIX, SCOUTER_TAG_PREFIX, k),
                    v.clone(),
                ));
            });
        });
        keyval_baggage
    }

    fn build_final_ctx(
        base_ctx: OtelContext,
        explicit_baggage: Vec<KeyValue>,
        py_baggage: Vec<KeyValue>,
    ) -> OtelContext {
        let mut merged: HashMap<String, opentelemetry::Value> = base_ctx
            .baggage()
            .iter()
            .map(|(k, v)| (k.to_string(), opentelemetry::Value::String(v.0.clone())))
            .collect();
        for kv in &py_baggage {
            merged.insert(kv.key.to_string(), kv.value.clone());
        }
        for kv in &explicit_baggage {
            merged.insert(kv.key.to_string(), kv.value.clone());
        }
        if merged.is_empty() {
            base_ctx
        } else {
            let all_items: Vec<KeyValue> = merged
                .into_iter()
                .map(|(k, v)| KeyValue::new(k, v))
                .collect();
            base_ctx.with_baggage(all_items)
        }
    }
}

#[pymethods]
impl BaseTracer {
    #[new]
    #[pyo3(signature = (
        scope_name,
        scope_version = None,
        schema_url = None,
        scope_attributes = None,
        default_attributes = None,
        queue = None,
    ))]
    #[instrument(skip_all)]
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'_>,
        scope_name: String,
        scope_version: Option<String>,
        schema_url: Option<String>,
        scope_attributes: Option<Bound<'_, PyAny>>,
        default_attributes: Option<Bound<'_, PyAny>>,
        queue: Option<Py<ScouterQueue>>,
    ) -> Result<Self, TraceError> {
        debug!("Creating new BaseTracer instance");

        // Determine the queue to use
        let final_queue = queue.or_else(|| {
            SCOUTER_QUEUE_STORE
                .read()
                .ok()
                .and_then(|guard| guard.as_ref().map(|q| q.clone_ref(py)))
        });

        // Convert Python attributes to OpenTelemetry KeyValue pairs
        let scope_attributes = py_obj_to_otel_keyvalue(py, scope_attributes)?;
        let default_attributes = py_obj_to_otel_keyvalue(py, default_attributes)?;

        let mut scope_builder = InstrumentationScope::builder(scope_name);
        if let Some(v) = scope_version {
            scope_builder = scope_builder.with_version(v);
        }
        if let Some(url) = schema_url {
            scope_builder = scope_builder.with_schema_url(url);
        }

        if !scope_attributes.is_empty() {
            scope_builder = scope_builder.with_attributes(scope_attributes);
        }

        let scope = scope_builder.build();
        let tracer = get_tracer_from_scope(scope)?;

        Ok(BaseTracer {
            tracer,
            queue: final_queue,
            default_attributes,
        })
    }

    pub fn set_scouter_queue(
        &mut self,
        py: Python<'_>,
        queue: Py<ScouterQueue>,
    ) -> Result<(), TraceError> {
        // if queue is not none, we set sample_ratio to 1.0 to ensure we override the drift profile sampling ratio
        // this mainly applies to agent evaluations as each insert checks if an eval record should be sampled
        // When we use tracing, we want to let tracing control the sampling decision, so we need to set the queue
        // sample ratio to 1.0 so that all events that the tracer samples are sent to the queue
        let bound_queue = queue.bind(py);
        bound_queue.call_method1("_set_sample_ratio", (1.0,))?;

        // update the store
        let mut store_guard = SCOUTER_QUEUE_STORE
            .write()
            .map_err(|e| TraceError::PoisonError(e.to_string()))?;
        *store_guard = Some(queue.clone_ref(py));

        self.queue = Some(queue);

        Ok(())
    }

    /// Start a span and set it as the current span
    /// # Arguments
    /// * `name` - The name of the span
    /// * `kind` - Optional kind of the span ("server", "client", "
    /// producer", "consumer", "internal")
    /// * `label` - Optional label for the span
    /// * `attributes` - Optional attributes as a dictionary
    /// * `baggage` - Optional baggage items as a dictionary
    /// * `tags` - Optional tags to prefix baggage items with as a dictionary
    /// * `parent_context_id` - Optional parent context ID to link the span to (this is automatically set if not provided)
    #[pyo3(signature = (
        name,
        context=None,
        kind=None,
        attributes=None,
        baggage=vec![],
        tags=vec![],
        label=None,
        parent_context_id=None,
        trace_id=None,
        span_id=None,
        remote_sampled=None,
        headers=None,
        links=None,
        start_time=None,
        record_exception=None,
        set_status_on_exception=None,
    ))]
    #[allow(clippy::too_many_arguments)]
    #[instrument(skip_all)]
    fn start_as_current_span(
        &self,
        py: Python<'_>,
        name: String,
        context: Option<&Bound<'_, PyAny>>, // Python OTel Context from auto-instrumentors
        kind: Option<&Bound<'_, PyAny>>,    // can be either SpanKind enum or otel span kind object
        attributes: Option<&Bound<'_, PyAny>>,
        baggage: Vec<HashMap<String, String>>,
        tags: Vec<HashMap<String, String>>,
        label: Option<String>,
        parent_context_id: Option<String>,
        trace_id: Option<String>,
        span_id: Option<String>,
        remote_sampled: Option<bool>, // only used if both trace_id and span_id are provided
        headers: Option<HashMap<String, String>>, // W3C traceparent dict
        links: Option<&Bound<'_, PyAny>>, // accepted for OTel compat, not yet used
        start_time: Option<i64>,      // accepted for OTel compat, not yet used
        record_exception: Option<bool>, // accepted for OTel compat, not yet used
        set_status_on_exception: Option<bool>, // accepted for OTel compat, not yet used
    ) -> Result<ActiveSpan, TraceError> {
        let _ = (links, start_time, record_exception, set_status_on_exception);
        let kind = parse_span_kind(kind)?;
        // Get parent context if available
        let parent_id = parent_context_id.or_else(|| get_current_context_id(py).ok().flatten());

        // Build the base context with the following priority:
        // 1. OTel Python Context object (from StarletteInstrumentor, ASGI middleware, etc.)
        // 2. W3C traceparent from headers dict
        // 3. Legacy explicit trace_id + span_id params
        // 4. Local in-process parent via context_id
        // 5. Root span (no parent)
        let base_ctx = if let Some(ctx) = context.and_then(|c| extract_otel_py_context(py, c)) {
            OtelContext::current().with_remote_span_context(ctx)
        } else if let Some(ref h) = headers {
            let extracted = TraceContextPropagator::new().extract(&HashMapExtractor(h));
            let sc = extracted.span().span_context().clone();
            if sc.is_valid() {
                OtelContext::current().with_remote_span_context(sc)
            } else if let (Some(tid), Some(sid)) = (h.get("trace_id"), h.get("span_id")) {
                // legacy scouter custom headers inside the headers dict
                let parsed_trace_id = TraceId::from_hex(tid)?;
                let parsed_span_id = SpanId::from_hex(sid)?;
                let remote_span_context = SpanContext::new(
                    parsed_trace_id,
                    parsed_span_id,
                    remote_sampled.map_or(TraceFlags::default(), |s| {
                        if s {
                            TraceFlags::SAMPLED
                        } else {
                            TraceFlags::NOT_SAMPLED
                        }
                    }),
                    true,
                    TraceState::default(),
                );
                OtelContext::current().with_remote_span_context(remote_span_context)
            } else {
                OtelContext::current()
            }
        } else if let (Some(tid), Some(sid)) = (&trace_id, &span_id) {
            // Both trace_id and span_id come from upstream service's headers
            let parsed_trace_id = TraceId::from_hex(tid)?;
            let parsed_span_id = SpanId::from_hex(sid)?; // This is the PARENT's span_id

            let remote_span_context = SpanContext::new(
                parsed_trace_id, // The shared trace ID
                parsed_span_id,  // The parent span's ID
                remote_sampled.map_or(TraceFlags::default(), |sampled| {
                    if sampled {
                        TraceFlags::SAMPLED
                    } else {
                        TraceFlags::NOT_SAMPLED
                    }
                }),
                true, // is_remote = true (parent is from different service)
                TraceState::default(),
            );

            OtelContext::current().with_remote_span_context(remote_span_context)
        } else if let Some(parent_id) = parent_id.as_ref() {
            // Use local parent (within same Python process)
            get_context_store()
                .get(parent_id)?
                .map(|parent_ctx| OtelContext::current().with_remote_span_context(parent_ctx))
                .unwrap_or_else(OtelContext::current)
        } else if let Some(sc) = get_otel_global_span_context(py) {
            // Fall back to any span attached via OTel's Python context system
            // (e.g. ASGI middleware, HTTPXInstrumentor, other OTel-compatible libraries)
            OtelContext::current().with_remote_span_context(sc)
        } else {
            // No parent — root span
            OtelContext::current()
        };

        let explicit_baggage = Self::create_baggage_items(&baggage, &tags);
        let py_baggage = extract_otel_py_baggage(py, context);
        let final_ctx = Self::build_final_ctx(base_ctx, explicit_baggage, py_baggage);

        let span_builder = self
            .tracer
            .span_builder(name.clone())
            .with_kind(kind.to_otel_span_kind());

        let mut span = BoxedSpan::new(span_builder.start_with_context(&self.tracer, &final_ctx));

        // Apply attributes — accepts both a flat dict and a list-of-dicts for OTel compat
        let explicit_attributes = if let Some(attrs) = attributes {
            py_obj_to_otel_keyvalue(py, Some(attrs.clone()))?
        } else {
            Vec::new()
        };
        explicit_attributes
            .iter()
            .cloned()
            .for_each(|kv| span.set_attribute(kv));

        // set default attributes from tracer configuration
        self.default_attributes.iter().for_each(|kv| {
            span.set_attribute(kv.clone());
        });

        if let Some(label) = label {
            span.set_attribute(KeyValue::new(SCOUTER_TRACING_LABEL, label));
        }

        let context_id = Self::set_context_id(self, &mut span)?;

        let inner = Arc::new(RwLock::new(ActiveSpanInner {
            context_id,
            parent_context_id: parent_id,
            span,
            context_token: None,
            queue: self.queue.as_ref().map(|q| q.clone_ref(py)),
            cleanup_complete: false,
        }));

        // set as current span
        self.set_current_span(py, &inner)?;

        Ok(ActiveSpan { inner })
    }

    /// Start a span without setting it as the current context span (OTel-compatible).
    ///
    /// Unlike `start_as_current_span`, this does **not** push the span onto the context
    /// stack. Use the returned `ActiveSpan` directly as a context manager or call
    /// `end()` manually.
    #[pyo3(signature = (
        name,
        context=None,
        kind=None,
        attributes=None,
        baggage=vec![],
        tags=vec![],
        label=None,
        parent_context_id=None,
        trace_id=None,
        span_id=None,
        remote_sampled=None,
        headers=None,
        links=None,
        start_time=None,
        record_exception=None,
        set_status_on_exception=None,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn start_span(
        &self,
        py: Python<'_>,
        name: String,
        context: Option<&Bound<'_, PyAny>>, // Python OTel Context from auto-instrumentors
        kind: Option<&Bound<'_, PyAny>>,
        attributes: Option<&Bound<'_, PyAny>>,
        baggage: Vec<HashMap<String, String>>,
        tags: Vec<HashMap<String, String>>,
        label: Option<String>,
        parent_context_id: Option<String>,
        trace_id: Option<String>,
        span_id: Option<String>,
        remote_sampled: Option<bool>,
        headers: Option<HashMap<String, String>>,
        links: Option<&Bound<'_, PyAny>>,
        start_time: Option<i64>,
        record_exception: Option<bool>,
        set_status_on_exception: Option<bool>,
    ) -> Result<ActiveSpan, TraceError> {
        let _ = (links, start_time, record_exception, set_status_on_exception);
        let kind = parse_span_kind(kind)?;
        let explicit_context = context.is_some();
        let parent_id = parent_context_id.or_else(|| {
            if explicit_context {
                None
            } else {
                get_current_context_id(py).ok().flatten()
            }
        });

        let base_ctx = if let Some(ctx) = context.and_then(|c| extract_otel_py_context(py, c)) {
            OtelContext::current().with_remote_span_context(ctx)
        } else if let Some(ref h) = headers {
            let extracted = TraceContextPropagator::new().extract(&HashMapExtractor(h));
            let sc = extracted.span().span_context().clone();
            if sc.is_valid() {
                OtelContext::current().with_remote_span_context(sc)
            } else if let (Some(tid), Some(sid)) = (h.get("trace_id"), h.get("span_id")) {
                let parsed_trace_id = TraceId::from_hex(tid)?;
                let parsed_span_id = SpanId::from_hex(sid)?;
                let remote_span_context = SpanContext::new(
                    parsed_trace_id,
                    parsed_span_id,
                    remote_sampled.map_or(TraceFlags::SAMPLED, |s| {
                        if s {
                            TraceFlags::SAMPLED
                        } else {
                            TraceFlags::NOT_SAMPLED
                        }
                    }),
                    true,
                    TraceState::default(),
                );
                OtelContext::current().with_remote_span_context(remote_span_context)
            } else {
                OtelContext::current()
            }
        } else if let (Some(tid), Some(sid)) = (&trace_id, &span_id) {
            let parsed_trace_id = TraceId::from_hex(tid)?;
            let parsed_span_id = SpanId::from_hex(sid)?;
            let remote_span_context = SpanContext::new(
                parsed_trace_id,
                parsed_span_id,
                remote_sampled.map_or(TraceFlags::default(), |sampled| {
                    if sampled {
                        TraceFlags::SAMPLED
                    } else {
                        TraceFlags::NOT_SAMPLED
                    }
                }),
                true,
                TraceState::default(),
            );
            OtelContext::current().with_remote_span_context(remote_span_context)
        } else if let Some(parent_id) = parent_id.as_ref() {
            get_context_store()
                .get(parent_id)?
                .map(|parent_ctx| OtelContext::current().with_remote_span_context(parent_ctx))
                .unwrap_or_else(OtelContext::current)
        } else if explicit_context {
            // Caller passed an explicit OTel context but it carried no valid span.
            // Skip get_otel_global_span_context to avoid attaching a stale parent
            // from the Python thread-local when the caller intentionally provided
            // a different (empty) context.
            OtelContext::current()
        } else if let Some(sc) = get_otel_global_span_context(py) {
            OtelContext::current().with_remote_span_context(sc)
        } else {
            OtelContext::current()
        };

        let explicit_baggage = Self::create_baggage_items(&baggage, &tags);
        let py_baggage = extract_otel_py_baggage(py, context);
        let final_ctx = Self::build_final_ctx(base_ctx, explicit_baggage, py_baggage);

        let span_builder = self
            .tracer
            .span_builder(name)
            .with_kind(kind.to_otel_span_kind());

        let mut span = BoxedSpan::new(span_builder.start_with_context(&self.tracer, &final_ctx));

        let explicit_attributes = if let Some(attrs) = attributes {
            py_obj_to_otel_keyvalue(py, Some(attrs.clone()))?
        } else {
            Vec::new()
        };
        explicit_attributes
            .iter()
            .cloned()
            .for_each(|kv| span.set_attribute(kv));

        self.default_attributes.iter().for_each(|kv| {
            span.set_attribute(kv.clone());
        });

        if let Some(label) = label {
            span.set_attribute(KeyValue::new(SCOUTER_TRACING_LABEL, label));
        }

        let context_id = Self::set_context_id(self, &mut span)?;

        let inner = Arc::new(RwLock::new(ActiveSpanInner {
            context_id,
            parent_context_id: parent_id,
            span,
            context_token: None, // not pushed onto the context stack
            queue: self.queue.as_ref().map(|q| q.clone_ref(py)),
            cleanup_complete: false,
        }));

        Ok(ActiveSpan { inner })
    }

    /// Special method that is used as a decorator to start a span around a function call
    /// This captures the function arguments and sets them as span attributes
    /// # Arguments
    /// * `func` - The function to be decorated
    /// * `name` - The name of the span
    /// * `kind` - Optional kind of the span ("server", "client", "
    /// producer", "consumer", "internal")
    /// * `label` - Optional label for the span
    /// * `attributes` - Optional attributes as a dictionary
    /// * `baggage` - Optional baggage items as a dictionary
    /// * `tags` - Optional tags to prefix baggage items with as a dictionary
    /// * `parent_context_id` - Optional parent context ID to link the span to (this is automatically set if not provided)
    /// * `max_length` - Maximum length of the serialized input (default: 1000)
    /// * `func_type` - Function type (sync or async)
    #[pyo3(name="_start_decorated_as_current_span", signature = (
        name,
        func,
        func_args,
        kind=None,
        attributes=None,
        baggage=vec![],
        tags=vec![],
        label=None,
        parent_context_id=None,
        trace_id=None,
        span_id=None,
        remote_sampled=None,
        max_length=1000,
        func_type=FunctionType::Sync,
        func_kwargs=None
    ))]
    #[allow(clippy::too_many_arguments)]
    fn start_decorated_as_current_span<'py>(
        &self,
        py: Python<'py>,
        name: String,
        func: &Bound<'py, PyAny>,
        func_args: &Bound<'_, PyTuple>,
        kind: Option<&Bound<'_, PyAny>>,
        attributes: Option<&Bound<'_, PyAny>>,
        baggage: Vec<HashMap<String, String>>,
        tags: Vec<HashMap<String, String>>,
        label: Option<String>,
        parent_context_id: Option<String>,
        trace_id: Option<String>,
        span_id: Option<String>,
        remote_sampled: Option<bool>,
        max_length: usize,
        func_type: FunctionType,
        func_kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Result<ActiveSpan, TraceError> {
        let mut span = self.start_as_current_span(
            py,
            name,
            None, // context
            kind,
            attributes,
            baggage,
            tags,
            label,
            parent_context_id,
            trace_id,
            span_id,
            remote_sampled,
            None, // headers
            None, // links
            None, // start_time
            None, // record_exception
            None, // set_status_on_exception
        )?;

        set_function_attributes(func, &mut span)?;

        set_function_type_attribute(&func_type, &mut span)?;

        span.set_input(
            &capture_function_arguments(py, func, func_args, func_kwargs)?,
            max_length,
        )?;

        Ok(span)
    }

    /// Get the current active span from context
    #[getter]
    pub fn current_span<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyAny>, TraceError> {
        let span = get_current_active_span(py)?;
        Ok(span)
    }

    pub fn shutdown(&self) -> Result<(), TraceError> {
        shutdown_tracer()
    }

    /// Enable run-scoped in-process span capture mode.
    pub fn enable_local_capture(&self, capture_run_id: String) -> Result<(), TraceError> {
        enable_capture_impl(&capture_run_id)
    }

    /// Disable run-scoped span capture, discarding any buffered spans for the run.
    pub fn disable_local_capture(&self, capture_run_id: String) -> Result<(), TraceError> {
        disable_capture_impl(&capture_run_id)
    }

    /// Drain and return locally captured spans for a run, clearing that buffer.
    /// Safe to call regardless of whether capture is currently enabled.
    pub fn drain_local_spans(
        &self,
        capture_run_id: String,
    ) -> Result<Vec<TraceSpanRecord>, TraceError> {
        drain_spans_impl(&capture_run_id)
    }

    /// Return captured spans matching the given trace IDs without draining the run buffer.
    /// Invalid hex trace IDs are skipped with a warning.
    pub fn get_local_spans_by_trace_ids(
        &self,
        capture_run_id: String,
        trace_ids: Vec<String>,
    ) -> Result<Vec<TraceSpanRecord>, TraceError> {
        let mut set = HashSet::new();
        for s in trace_ids {
            match ScouterTraceId::from_hex(&s) {
                Ok(id) => {
                    set.insert(id);
                }
                Err(_) => {
                    warn!("Invalid trace ID format, skipping: {}", s);
                }
            }
        }
        get_spans_by_trace_ids(&capture_run_id, &set)
    }
}

impl BaseTracer {
    fn set_current_span(
        &self,
        py: Python<'_>,
        inner: &Arc<RwLock<ActiveSpanInner>>,
    ) -> Result<(), TraceError> {
        let py_span = Py::new(
            py,
            ActiveSpan {
                inner: inner.clone(),
            },
        )?;
        let token = set_current_span(py, py_span.bind(py).clone())?;
        inner
            .write()
            .map_err(|e| TraceError::PoisonError(e.to_string()))?
            .context_token = Some(token);
        Ok(())
    }

    fn set_context_id(&self, span: &mut BoxedSpan) -> Result<String, TraceError> {
        let context_id = format!("span_{}", create_uuid7());
        Self::setup_trace_metadata(self, span)?;
        get_context_store().set(context_id.clone(), span.span_context().clone())?;
        Ok(context_id)
    }
}

/// Helper function to force flush the tracer provider
#[pyfunction]
pub fn flush_tracer() -> Result<(), TraceError> {
    let provider_arc = get_tracer_provider()?.ok_or_else(|| {
        TraceError::InitializationError(
            "Tracer provider not initialized or already shut down".to_string(),
        )
    })?;

    provider_arc.force_flush()?;
    Ok(())
}

#[pyfunction]
pub fn reset_tracer_provider() -> Result<(), TraceError> {
    let mut guard = TRACER_PROVIDER_STORE
        .write()
        .map_err(|e| TraceError::PoisonError(e.to_string()))?;
    *guard = None;
    Ok(())
}

#[pyfunction]
pub fn shutdown_tracer() -> Result<(), TraceError> {
    info!("Shutting down tracer");

    let provider_arc = {
        let mut store_guard = TRACER_PROVIDER_STORE
            .write()
            .map_err(|e| TraceError::PoisonError(e.to_string()))?;

        store_guard.take()
    };

    if let Some(provider) = provider_arc {
        match Arc::try_unwrap(provider) {
            Ok(provider) => match provider.shutdown() {
                Ok(_) => (),
                Err(e) => {
                    tracing::warn!("Failed to shut down tracer provider: {}", e);
                }
            },
            Err(arc) => match arc.shutdown() {
                Ok(_) => (),
                Err(e) => {
                    tracing::warn!("Failed to shut down tracer provider: {}", e);
                }
            },
        }
    } else {
        tracing::warn!("Tracer provider was already shut down or never initialized.");
    }

    get_trace_metadata_store().clear_all()?;

    // clear global scouter queue
    let mut queue_store_guard = SCOUTER_QUEUE_STORE
        .write()
        .map_err(|e| TraceError::PoisonError(e.to_string()))?;
    *queue_store_guard = None;

    scouter_types::span_capture::disable_all_captures();

    Ok(())
}

// ── Local span capture helpers ─────────────────────────────────────────────

fn enable_capture_impl(capture_run_id: &str) -> Result<(), TraceError> {
    scouter_types::span_capture::enable_capture(capture_run_id);
    info!(
        capture_run_id,
        "Local span capture enabled — spans will be buffered in-process"
    );
    Ok(())
}

fn disable_capture_impl(capture_run_id: &str) -> Result<(), TraceError> {
    scouter_types::span_capture::disable_capture(capture_run_id);
    Ok(())
}

pub fn drain_spans_impl(capture_run_id: &str) -> Result<Vec<TraceSpanRecord>, TraceError> {
    Ok(scouter_types::span_capture::drain_captured_spans(
        capture_run_id,
    ))
}

#[pyfunction]
pub fn enable_local_span_capture(capture_run_id: String) -> Result<(), TraceError> {
    enable_capture_impl(&capture_run_id)
}

#[pyfunction]
pub fn disable_local_span_capture(capture_run_id: String) -> Result<(), TraceError> {
    disable_capture_impl(&capture_run_id)
}

#[pyfunction]
pub fn drain_local_span_capture(
    capture_run_id: String,
) -> Result<Vec<TraceSpanRecord>, TraceError> {
    drain_spans_impl(&capture_run_id)
}

/// Returns clones of spans matching the given trace_ids.
/// Does NOT drain the buffer — call drain_spans_impl() after all evaluations.
pub fn get_spans_by_trace_ids(
    capture_run_id: &str,
    trace_ids: &HashSet<ScouterTraceId>,
) -> Result<Vec<TraceSpanRecord>, TraceError> {
    Ok(scouter_types::span_capture::get_captured_spans_by_trace_ids(capture_run_id, trace_ids))
}

/// Returns a clone of all captured spans without draining.
pub fn get_all_captured_spans(capture_run_id: &str) -> Result<Vec<TraceSpanRecord>, TraceError> {
    Ok(scouter_types::span_capture::get_all_captured_spans(
        capture_run_id,
    ))
}

fn get_tracer_from_scope(scope: InstrumentationScope) -> Result<SdkTracer, TraceError> {
    let provider_arc = get_tracer_provider()?.ok_or_else(|| {
        TraceError::InitializationError(
            "Tracer provider not initialized or already shut down".to_string(),
        )
    })?;

    Ok(provider_arc.tracer_with_scope(scope))
}

/// Check the global OTel Python context for a current span and return its `SpanContext`.
///
/// When third-party instrumentors (ASGI middleware, HTTPXInstrumentor, etc.) attach a span
/// via `opentelemetry.context.attach(trace.set_span_in_context(span))`, that span lives in
/// OTel's Python context system — not in Scouter's own `_otel_current_span` ContextVar.
/// This helper bridges the two, so Scouter spans automatically parent to any span set by
/// an OTel-compatible instrumentor.
fn get_otel_global_span_context(py: Python<'_>) -> Option<SpanContext> {
    let trace_mod = py.import("opentelemetry.trace").ok()?;
    let current_span = trace_mod.call_method0("get_current_span").ok()?;
    let span_ctx = current_span.call_method0("get_span_context").ok()?;
    let sc = SpanContext::from_py_span_context(&span_ctx).ok()?;
    if sc.is_valid() { Some(sc) } else { None }
}

/// Extract a `SpanContext` from a Python OTel `Context` object.
///
/// Auto-instrumentors (ASGI, HTTPX, gRPC) call `tracer.start_span(context=ctx)`
/// where `ctx` is an `opentelemetry.context.Context` dict containing the parent
/// span extracted from incoming wire headers. This helper unpacks it into a Rust
/// `SpanContext` so we can use it as the remote parent.
fn extract_otel_py_context(py: Python<'_>, context: &Bound<'_, PyAny>) -> Option<SpanContext> {
    let trace_mod = py.import("opentelemetry.trace").ok()?;
    let current_span = trace_mod
        .call_method1("get_current_span", (context,))
        .ok()?;
    let span_ctx = current_span.call_method0("get_span_context").ok()?;
    let sc = SpanContext::from_py_span_context(&span_ctx).ok()?;
    if sc.is_valid() { Some(sc) } else { None }
}

/// NOTE: Baggage values are not sanitized or filtered. Callers are responsible for ensuring
/// no PII or sensitive data is placed in OTel baggage, as all key-value pairs propagate to
/// every child span and downstream exporters (e.g. the Scouter trace backend).
fn extract_otel_py_baggage(py: Python<'_>, context: Option<&Bound<'_, PyAny>>) -> Vec<KeyValue> {
    let Ok(baggage_mod) = py.import("opentelemetry.baggage") else {
        return Vec::new();
    };

    let result = match context {
        Some(ctx) => {
            let kwargs = PyDict::new(py);
            if kwargs.set_item("context", ctx).is_err() {
                return Vec::new();
            }
            baggage_mod.call_method("get_all", (), Some(&kwargs))
        }
        None => baggage_mod.call_method0("get_all"),
    };

    let Ok(baggage_obj) = result else {
        return Vec::new();
    };

    let Ok(items) = baggage_obj.call_method0("items") else {
        return Vec::new();
    };

    let Ok(iter) = items.try_iter() else {
        return Vec::new();
    };

    iter.flatten()
        .filter_map(|item| item.extract::<(String, String)>().ok())
        .map(|(k, v)| KeyValue::new(k, v))
        .collect()
}

#[pyfunction]
pub fn get_tracing_headers_from_current_span(
    py: Python<'_>,
) -> Result<HashMap<String, String>, TraceError> {
    let current_span_py = get_current_active_span(py)?;

    let active_span_ref = current_span_py
        .extract::<PyRef<ActiveSpan>>()
        .map_err(|e| TraceError::DowncastError(format!("Failed to extract ActiveSpan: {}", e)))?;

    // Get the stored context that includes both span and baggage
    let context_to_propagate = {
        let inner_guard = active_span_ref
            .inner
            .read()
            .map_err(|e| TraceError::PoisonError(e.to_string()))?;

        inner_guard.span.span_context().clone()
    };

    // Inject into headers
    // add trace_id and span_id for easier access (legacy format, kept for backward compat)
    let mut headers: HashMap<String, String> = HashMap::new();
    headers.insert(
        "trace_id".to_string(),
        context_to_propagate.trace_id().to_string(),
    );
    headers.insert(
        "span_id".to_string(),
        context_to_propagate.span_id().to_string(),
    );

    // get is_sampled flag
    let is_sampled = &context_to_propagate.trace_flags().is_sampled().to_string();
    headers.insert("is_sampled".to_string(), is_sampled.to_string());

    // Inject W3C traceparent + tracestate so HTTPXInstrumentor / StarletteInstrumentor
    // can interop with Scouter spans transparently.
    let otel_ctx = OtelContext::current().with_remote_span_context(context_to_propagate);
    TraceContextPropagator::new().inject_context(&otel_ctx, &mut HashMapInjector(&mut headers));

    Ok(headers)
}

/// Extract span context fields from a headers dict, supporting both W3C `traceparent`
/// and the legacy Scouter `trace_id`/`span_id` formats.
///
/// Returns `None` if no valid trace context is found.
#[pyfunction]
pub fn extract_span_context_from_headers(
    headers: HashMap<String, String>,
) -> Result<Option<HashMap<String, String>>, TraceError> {
    // Try W3C traceparent first
    let ctx = TraceContextPropagator::new().extract(&HashMapExtractor(&headers));
    let sc = ctx.span().span_context().clone();
    if sc.is_valid() {
        let mut out = HashMap::new();
        out.insert("trace_id".to_string(), sc.trace_id().to_string());
        out.insert("span_id".to_string(), sc.span_id().to_string());
        out.insert(
            "is_sampled".to_string(),
            sc.trace_flags().is_sampled().to_string(),
        );
        return Ok(Some(out));
    }

    // Fallback: legacy custom headers — validate hex before returning
    if let (Some(tid), Some(sid)) = (headers.get("trace_id"), headers.get("span_id")) {
        if TraceId::from_hex(tid).is_err() || SpanId::from_hex(sid).is_err() {
            return Ok(None);
        }
        let mut out = HashMap::new();
        out.insert("trace_id".to_string(), tid.clone());
        out.insert("span_id".to_string(), sid.clone());
        out.insert(
            "is_sampled".to_string(),
            headers
                .get("is_sampled")
                .cloned()
                .unwrap_or_else(|| "false".to_string()),
        );
        return Ok(Some(out));
    }

    Ok(None)
}

fn is_tracer_initialized() -> bool {
    TRACER_PROVIDER_STORE
        .read()
        .map(|guard| guard.is_some())
        .unwrap_or(false)
}

/// Helper method for setting attributes on the current active span if it exists
/// Mainly used in Opsml to log attributes from various places without needing to pass around the span object
/// # Arguments
/// * `py` - The Python GIL token
/// * `key` - The attribute key
/// * `value` - The attribute value
pub fn try_set_span_attribute(py: Python<'_>, key: &str, value: &str) -> Result<bool, TraceError> {
    // Check if tracer is initialized
    if !is_tracer_initialized() {
        return Ok(false);
    }

    // Try to get current span
    let span = match get_current_active_span(py) {
        Ok(s) => s,
        Err(_) => return Ok(false),
    };

    span.call_method1("set_attribute", (key, value))?;
    Ok(true)
}

#[cfg(test)]
mod server_otlp_tests {
    use super::*;

    #[test]
    fn disabled_server_otlp_returns_no_handle() {
        let settings = OtelSettings {
            enabled: false,
            endpoint: "http://localhost:4317".to_string(),
            protocol: ScouterOtelProtocol::Grpc,
            service_name: "scouter-server".to_string(),
            sample_ratio: 1.0,
            export_timeout_secs: 10,
        };

        let handle = init_server_otlp_tracing(&settings).unwrap();
        assert!(handle.is_none());
    }
}

#[cfg(test)]
mod capture_tests {
    use super::*;
    use scouter_types::trace::{Attribute, SCOUTER_EVAL_RUN_ID_ATTR};
    use serde_json::Value;
    use std::collections::BTreeSet;
    use std::sync::atomic::Ordering;

    const RUN_ID: &str = "capture_test_run";

    fn span_for_run(trace_id: ScouterTraceId, capture_run_id: &str) -> TraceSpanRecord {
        TraceSpanRecord {
            trace_id,
            attributes: vec![Attribute {
                key: SCOUTER_EVAL_RUN_ID_ATTR.to_string(),
                value: Value::String(capture_run_id.to_string()),
            }],
            ..TraceSpanRecord::default()
        }
    }

    fn reset() {
        scouter_types::span_capture::disable_all_captures();
        let _ = get_trace_metadata_store().clear_all();
    }

    #[test]
    fn test_enable_sets_capturing_true() {
        reset();
        enable_capture_impl(RUN_ID).unwrap();
        assert!(CAPTURING.load(Ordering::Acquire));
        reset();
    }

    #[test]
    fn test_disable_sets_capturing_false() {
        reset();
        enable_capture_impl(RUN_ID).unwrap();
        disable_capture_impl(RUN_ID).unwrap();
        assert!(!CAPTURING.load(Ordering::Acquire));
        reset();
    }

    #[test]
    fn test_drain_clears_and_returns_empty() {
        reset();
        enable_capture_impl(RUN_ID).unwrap();
        let drained = drain_spans_impl(RUN_ID).unwrap();
        assert!(drained.is_empty());
        assert!(CAPTURING.load(Ordering::Acquire)); // still capturing after drain
        reset();
    }

    #[test]
    fn test_drain_returns_empty_when_capture_off() {
        reset();
        let result = drain_spans_impl(RUN_ID).unwrap();
        assert!(result.is_empty());
        reset();
    }

    #[test]
    fn test_drain_returns_and_clears_populated_buffer() {
        reset();
        enable_capture_impl(RUN_ID).unwrap();
        scouter_types::span_capture::buffer_captured_spans(vec![span_for_run(
            ScouterTraceId::from_bytes([1; 16]),
            RUN_ID,
        )]);
        let drained = drain_spans_impl(RUN_ID).unwrap();
        assert_eq!(drained.len(), 1);
        assert!(scouter_types::span_capture::get_all_captured_spans(RUN_ID).is_empty());
        reset();
    }

    #[test]
    fn test_enable_clears_existing_buffer() {
        reset();
        enable_capture_impl(RUN_ID).unwrap();
        scouter_types::span_capture::buffer_captured_spans(vec![span_for_run(
            ScouterTraceId::from_bytes([1; 16]),
            RUN_ID,
        )]);
        enable_capture_impl(RUN_ID).unwrap();
        assert!(scouter_types::span_capture::get_all_captured_spans(RUN_ID).is_empty());
        reset();
    }

    #[test]
    fn test_get_spans_by_trace_ids_filters_without_drain() {
        reset();
        enable_capture_impl(RUN_ID).unwrap();
        let id_a = ScouterTraceId::from_bytes([1u8; 16]);
        let id_b = ScouterTraceId::from_bytes([2u8; 16]);
        scouter_types::span_capture::buffer_captured_spans(vec![
            span_for_run(id_a, RUN_ID),
            span_for_run(id_b, RUN_ID),
        ]);
        let set = HashSet::from([id_a]);
        let result = get_spans_by_trace_ids(RUN_ID, &set).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].trace_id, id_a);
        // buffer not drained
        assert_eq!(
            scouter_types::span_capture::get_all_captured_spans(RUN_ID).len(),
            2
        );
        reset();
    }

    #[test]
    fn test_buffer_overflow_drops_excess() {
        reset();
        enable_capture_impl(RUN_ID).unwrap();
        let spans = (0..=CAPTURE_BUFFER_MAX)
            .map(|_| span_for_run(ScouterTraceId::from_bytes([1; 16]), RUN_ID))
            .collect();
        scouter_types::span_capture::buffer_captured_spans(spans);
        assert_eq!(
            scouter_types::span_capture::get_all_captured_spans(RUN_ID).len(),
            CAPTURE_BUFFER_MAX
        );
        reset();
    }

    #[test]
    fn test_drain_returns_spans_for_scope_even_when_capture_flag_false() {
        reset();
        enable_capture_impl(RUN_ID).unwrap();
        scouter_types::span_capture::buffer_captured_spans(vec![span_for_run(
            ScouterTraceId::from_bytes([1; 16]),
            RUN_ID,
        )]);
        CAPTURING.store(false, Ordering::Release);
        let result = drain_spans_impl(RUN_ID).unwrap();
        assert_eq!(result.len(), 1); // drain is unconditional
        assert!(scouter_types::span_capture::get_all_captured_spans(RUN_ID).is_empty());
        reset();
    }

    #[test]
    fn observability_contract_route_contract_preserves_in_scope_trace_endpoints() {
        use observability_contract::routes;

        assert_eq!(routes::TRACE_PAGINATED_METHOD, "POST");
        assert_eq!(routes::TRACE_PAGINATED_PATH, "{prefix}/trace/paginated");
        assert_eq!(routes::TRACE_SPANS_METHOD, "GET");
        assert_eq!(routes::TRACE_SPANS_PATH, "{prefix}/trace/spans");
        assert_eq!(routes::TRACE_METRICS_METHOD, "POST");
        assert_eq!(routes::TRACE_METRICS_PATH, "{prefix}/trace/metrics");
        assert_eq!(routes::V1_TRACE_SPANS_METHOD, "GET");
        assert_eq!(routes::V1_TRACE_SPANS_PATH, "{prefix}/v1/traces/{id}/spans");
        assert_eq!(routes::V1_TRACES_METHOD, "POST");
        assert_eq!(routes::V1_TRACES_PATH, "{prefix}/v1/traces");
    }

    #[test]
    fn observability_contract_span_names_are_complete_and_unique() {
        use observability_contract::{SPAN_NAMES, span_names};

        let expected = [
            span_names::PAGINATED_TRACES_HANDLER,
            span_names::GET_TRACE_SPANS_HANDLER,
            span_names::TRACE_METRICS_HANDLER,
            span_names::GET_TRACE_SPANS_BY_ID_HANDLER,
            span_names::V1_OTEL_TRACES_HANDLER,
            span_names::TRACE_QUERY_PAGINATED,
            span_names::TRACE_QUERY_METRICS,
            span_names::TRACE_QUERY_SPANS,
            span_names::DF_TABLE_RESOLVE,
            span_names::DF_LOGICAL_BUILD,
            span_names::DF_PHYSICAL_PLAN,
            span_names::DF_COLLECT,
            span_names::ARROW_CONVERT,
            span_names::TRACE_TREE_BUILD,
            span_names::DELTA_TABLE_LOAD,
            span_names::DELTA_SNAPSHOT_REFRESH,
            span_names::DELTA_CATALOG_SWAP,
            span_names::DELTA_OPTIMIZE,
            span_names::UPDATE_INCREMENTAL,
            span_names::OBJECT_STORE_REQUEST,
        ];

        assert_eq!(SPAN_NAMES, expected);
        assert_unique(SPAN_NAMES);
    }

    #[test]
    fn observability_contract_metric_contracts_are_complete_and_unique() {
        use observability_contract::{METRIC_CONTRACTS, MetricKind, metric_names};

        let names: Vec<&str> = METRIC_CONTRACTS.iter().map(|metric| metric.name).collect();
        assert_unique(&names);

        let expected = [
            metric_names::TRACE_QUERY_DURATION_MS,
            metric_names::TRACE_DF_COLLECT_DURATION_MS,
            metric_names::TRACE_DF_PLAN_DURATION_MS,
            metric_names::TRACE_DELTA_REFRESH_DURATION_MS,
            metric_names::TRACE_OBJECT_STORE_REQUESTS_TOTAL,
            metric_names::TRACE_OBJECT_STORE_REQUEST_DURATION_MS,
            metric_names::TRACE_OBJECT_STORE_BYTES_TOTAL,
            metric_names::TRACE_CACHE_HITS_TOTAL,
            metric_names::TRACE_CACHE_MISSES_TOTAL,
            metric_names::TRACE_UNBOUNDED_LOOKUP_TOTAL,
            metric_names::REFRESH_ON_REQUEST_PATH_TOTAL,
        ];
        assert_eq!(names, expected);

        let refresh_metric = METRIC_CONTRACTS
            .iter()
            .find(|metric| metric.name == metric_names::REFRESH_ON_REQUEST_PATH_TOTAL)
            .unwrap();
        assert_eq!(refresh_metric.kind, MetricKind::Counter);
        assert_eq!(refresh_metric.labels, ["engine"]);
    }

    #[test]
    fn observability_contract_attribute_keys_are_complete_and_unique() {
        use observability_contract::{
            OBJECT_STORE_ATTRIBUTE_KEYS, TRACE_QUERY_ATTRIBUTE_KEYS, attribute_keys,
        };

        let expected_trace_keys = [
            attribute_keys::TRACE_QUERY_ENDPOINT,
            attribute_keys::TRACE_QUERY_KIND,
            attribute_keys::TRACE_QUERY_HAS_START_TIME,
            attribute_keys::TRACE_QUERY_HAS_END_TIME,
            attribute_keys::TRACE_QUERY_WINDOW_MS,
            attribute_keys::TRACE_QUERY_LIMIT,
            attribute_keys::TRACE_QUERY_OFFSET,
            attribute_keys::TRACE_QUERY_TRACE_ID_PRESENT,
            attribute_keys::TRACE_QUERY_UNBOUNDED,
            attribute_keys::TRACE_QUERY_CACHE_HIT,
            attribute_keys::TRACE_QUERY_CACHE_NAME,
            attribute_keys::TRACE_QUERY_RESULT_ROWS,
            attribute_keys::TRACE_QUERY_RESULT_BYTES_ESTIMATE,
            attribute_keys::TRACE_QUERY_TABLE_VERSION,
            attribute_keys::TRACE_QUERY_STORAGE_BACKEND,
            attribute_keys::TRACE_QUERY_REFRESH_ORIGIN,
        ];
        assert_eq!(TRACE_QUERY_ATTRIBUTE_KEYS, expected_trace_keys);
        assert_unique(TRACE_QUERY_ATTRIBUTE_KEYS);

        let expected_object_store_keys = [
            attribute_keys::OBJECT_STORE_BACKEND,
            attribute_keys::OBJECT_STORE_OPERATION,
            attribute_keys::OBJECT_STORE_PATH_KIND,
            attribute_keys::OBJECT_STORE_PATH_HASH,
            attribute_keys::OBJECT_STORE_RANGE_START,
            attribute_keys::OBJECT_STORE_RANGE_LEN,
            attribute_keys::OBJECT_STORE_CACHE_HIT,
            attribute_keys::OBJECT_STORE_STATUS,
            attribute_keys::OBJECT_STORE_ERROR_KIND,
            attribute_keys::PARQUET_FOOTER_CANDIDATE,
        ];
        assert_eq!(OBJECT_STORE_ATTRIBUTE_KEYS, expected_object_store_keys);
        assert_unique(OBJECT_STORE_ATTRIBUTE_KEYS);
    }

    fn assert_unique(values: &[&str]) {
        let unique = values.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(unique.len(), values.len());
    }
}
