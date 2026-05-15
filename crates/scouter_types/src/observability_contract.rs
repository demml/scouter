/// Stable OLAP observability contract.
///
/// Server, dataframe, Delta, and object-store instrumentation use these constants
/// so baseline artifacts remain comparable across optimization phases.
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
    pub const TRACE_QUERY_PADDING_SECS: &str = "trace.query.padding_secs";
    pub const TRACE_QUERY_HINT_WINDOW_SECS: &str = "trace.query.hint_window_secs";

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
    pub const TRACE_OBJECT_STORE_REQUESTS_TOTAL: &str = "scouter_trace_object_store_requests_total";
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
    attribute_keys::TRACE_QUERY_PADDING_SECS,
    attribute_keys::TRACE_QUERY_HINT_WINDOW_SECS,
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
