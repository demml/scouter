use crate::error::DataFrameError;
use arrow::array::AsArray;
use arrow::array::{BooleanBuilder, StringArray};
use arrow::datatypes::DataType;
use arrow::datatypes::UInt32Type;
use arrow_array::Array;
use arrow_array::RecordBatch;
use arrow_array::StringViewArray;
use arrow_array::types::Float64Type;
use arrow_array::types::TimestampNanosecondType;
use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use datafusion::error::{DataFusionError, Result};
use datafusion::logical_expr::ScalarFunctionArgs;
use datafusion::logical_expr::{
    ColumnarValue, Expr, ScalarUDF, ScalarUDFImpl, Signature, TypeSignature, Volatility,
};
use datafusion::prelude::DataFrame;
use datafusion::scalar::ScalarValue;
use deltalake::DeltaResult;
use deltalake::logstore::{
    LogStore, LogStoreFactory, ObjectStoreRef, StorageConfig, default_logstore, logstore_factories,
};
use futures::Stream;
use futures::StreamExt;
use futures::stream::BoxStream;
use object_store::path::Path;
use object_store::{
    CopyOptions, Error as ObjectStoreError, GetOptions, GetRange, GetResult, ListResult,
    MultipartUpload, ObjectMeta, ObjectStore, PutMultipartOptions, PutOptions, PutPayload,
    PutResult, Result as ObjectStoreResult,
};
use scouter_types::{BinnedMetric, BinnedMetricStats, BinnedMetrics};
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Instant;
use tracing::{Instrument, Span, field};
use tracing::{debug, error, instrument};
use url::Url;

/// Now that we have at least 2 metric types that calculate avg, lower_bound, and upper_bound as part of their stats,
/// it makes sense to implement a generic trait that we can use.
pub struct ParquetHelper {}

impl ParquetHelper {
    #[instrument(skip_all)]
    pub fn extract_feature_array(batch: &RecordBatch) -> Result<&StringViewArray, DataFrameError> {
        let feature_array = batch
            .column_by_name("feature")
            .ok_or_else(|| {
                error!("Missing 'feature' field in RecordBatch");
                DataFrameError::MissingFieldError("feature")
            })?
            .as_string_view_opt()
            .ok_or_else(|| {
                error!("Failed to downcast 'feature' field to StringViewArray");
                DataFrameError::DowncastError("StringViewArray")
            })?;
        Ok(feature_array)
    }

    #[instrument(skip_all)]
    pub fn extract_created_at(batch: &RecordBatch) -> Result<Vec<DateTime<Utc>>, DataFrameError> {
        let created_at_list = batch
            .column_by_name("created_at")
            .ok_or_else(|| {
                error!("Missing 'created_at' field in RecordBatch");
                DataFrameError::MissingFieldError("created_at")
            })?
            .as_list_opt::<i32>()
            .ok_or_else(|| {
                error!("Failed to downcast 'created_at' field to ListArray");
                DataFrameError::DowncastError("ListArray")
            })?;

        let created_at_array = created_at_list.value(0);
        Ok(created_at_array
            .as_primitive::<TimestampNanosecondType>()
            .iter()
            .filter_map(|ts| ts.map(|t| Utc.timestamp_nanos(t)))
            .collect())
    }
}
pub struct BinnedMetricsExtractor {}

impl BinnedMetricsExtractor {
    #[instrument(skip_all)]
    fn extract_stats(batch: &RecordBatch) -> Result<Vec<BinnedMetricStats>, DataFrameError> {
        let stats_list = batch
            .column_by_name("stats")
            .ok_or_else(|| {
                error!("Missing 'stats' field in RecordBatch");
                DataFrameError::MissingFieldError("stats")
            })?
            .as_list_opt::<i32>()
            .ok_or_else(|| {
                error!("Failed to downcast 'stats' field to ListArray");
                DataFrameError::DowncastError("ListArray")
            })?
            .value(0);

        let stats_structs = stats_list.as_struct_opt().ok_or_else(|| {
            error!("Failed to downcast 'stats' field to StructArray");
            DataFrameError::DowncastError("StructArray")
        })?;

        let avg_array = stats_structs
            .column_by_name("avg")
            .ok_or_else(|| DataFrameError::MissingFieldError("avg"))
            .inspect_err(|e| error!("Failed to get 'avg' field from stats: {:?}", e))?
            .as_primitive_opt::<Float64Type>()
            .ok_or_else(|| DataFrameError::DowncastError("Float64Array"))?;

        let lower_bound_array = stats_structs
            .column_by_name("lower_bound")
            .ok_or_else(|| DataFrameError::MissingFieldError("lower_bound"))
            .inspect_err(|e| error!("Failed to get 'lower_bound' field from stats: {:?}", e))?
            .as_primitive_opt::<Float64Type>()
            .ok_or_else(|| DataFrameError::DowncastError("Float64Array"))?;

        let upper_bound_array = stats_structs
            .column_by_name("upper_bound")
            .ok_or_else(|| DataFrameError::MissingFieldError("upper_bound"))
            .inspect_err(|e| error!("Failed to get 'upper_bound' field from stats: {:?}", e))?
            .as_primitive_opt::<Float64Type>()
            .ok_or_else(|| DataFrameError::DowncastError("Float64Array"))?;

        Ok((0..stats_structs.len())
            .map(|i| BinnedMetricStats {
                avg: avg_array.value(i),
                lower_bound: lower_bound_array.value(i),
                upper_bound: upper_bound_array.value(i),
            })
            .collect())
    }

    #[instrument(skip_all)]
    fn process_metric_record_batch(batch: &RecordBatch) -> Result<BinnedMetric, DataFrameError> {
        debug!("Processing metric record batch");

        let metric_column = batch.column_by_name("metric").ok_or_else(|| {
            error!("Missing 'metric' field in RecordBatch");
            DataFrameError::MissingFieldError("metric")
        })?;

        // Handle both Dictionary and plain string types
        let metric_name = if let Some(dict_array) = metric_column.as_dictionary_opt::<UInt32Type>()
        {
            // Dictionary-encoded string (e.g., from GenAI task_id)
            let values = dict_array.values();
            let string_values = values.as_string_opt::<i32>().ok_or_else(|| {
                error!("Failed to downcast dictionary values to StringArray");
                DataFrameError::DowncastError("StringArray")
            })?;
            let key = dict_array.key(0).ok_or_else(|| {
                error!("Failed to get key from dictionary array");
                DataFrameError::MissingFieldError("dictionary key")
            })?;
            string_values.value(key).to_string()
        } else if let Some(string_view_array) = metric_column.as_string_view_opt() {
            // StringView type
            string_view_array.value(0).to_string()
        } else if let Some(string_array) = metric_column.as_string_opt::<i32>() {
            // Plain string type
            string_array.value(0).to_string()
        } else {
            error!("Failed to downcast 'metric' field to any supported string type");
            return Err(DataFrameError::DowncastError("String type"));
        };

        let created_at_list = ParquetHelper::extract_created_at(batch)?;
        let stats = Self::extract_stats(batch)?;

        Ok(BinnedMetric {
            metric: metric_name,
            created_at: created_at_list,
            stats,
        })
    }

    /// Convert a DataFrame to BinnedMetrics.
    ///
    /// # Arguments
    /// * `df` - The DataFrame to convert
    ///
    /// # Returns
    /// * `BinnedMetrics` - The converted BinnedMetrics
    #[instrument(skip_all)]
    pub async fn dataframe_to_binned_metrics(
        df: DataFrame,
    ) -> Result<BinnedMetrics, DataFrameError> {
        debug!("Converting DataFrame to binned metrics");

        let batches = df.collect().await?;

        let metrics: Vec<BinnedMetric> = batches
            .iter()
            .map(Self::process_metric_record_batch)
            .collect::<Result<Vec<_>, _>>()
            .inspect_err(|e| {
                error!("Failed to process metric record batch: {:?}", e);
            })?;

        Ok(BinnedMetrics::from_vec(metrics))
    }
}

pub(crate) const OBJECT_STORE_SPAN_NAME: &str = "object_store.request";
pub(crate) const OBJECT_STORE_STATUS_ATTR: &str = "object_store.status";
pub(crate) const OBJECT_STORE_ERROR_KIND_ATTR: &str = "object_store.error.kind";

pub(crate) const OBJECT_STORE_OPERATION_LIST: &str = "list";
pub(crate) const OBJECT_STORE_OPERATION_LIST_WITH_DELIMITER: &str = "list_with_delimiter";
pub(crate) const OBJECT_STORE_OPERATION_HEAD: &str = "head";
pub(crate) const OBJECT_STORE_OPERATION_GET: &str = "get";
pub(crate) const OBJECT_STORE_OPERATION_GET_RANGE: &str = "get_range";
pub(crate) const OBJECT_STORE_OPERATION_PUT: &str = "put";
pub(crate) const OBJECT_STORE_OPERATION_DELETE: &str = "delete";
pub(crate) const OBJECT_STORE_OPERATION_COPY: &str = "copy";

pub(crate) const OBJECT_STORE_PATH_KIND_DELTA_LOG: &str = "delta_log";
pub(crate) const OBJECT_STORE_PATH_KIND_PARQUET_DATA: &str = "parquet_data";
pub(crate) const OBJECT_STORE_PATH_KIND_CHECKPOINT: &str = "checkpoint";
pub(crate) const OBJECT_STORE_PATH_KIND_UNKNOWN: &str = "unknown";

const TRACE_OBJECT_STORE_REQUESTS_TOTAL: &str = "scouter_trace_object_store_requests_total";
const TRACE_OBJECT_STORE_REQUEST_DURATION_MS: &str =
    "scouter_trace_object_store_request_duration_ms";
const TRACE_OBJECT_STORE_BYTES_TOTAL: &str = "scouter_trace_object_store_bytes_total";
const CACHE_HIT_UNKNOWN: &str = "unknown";
const STATUS_OK: &str = "ok";
const STATUS_ERROR: &str = "error";
const STATUS_DROPPED: &str = "dropped";
const PARQUET_FOOTER_CANDIDATE_MAX_BYTES: u64 = 2 * 1024 * 1024;

pub(crate) fn classify_object_path(location: &Path) -> &'static str {
    let path = location.as_ref();
    let file_name = path.rsplit('/').next().unwrap_or(path);

    if path.ends_with("_delta_log/_last_checkpoint") || file_name.contains(".checkpoint.") {
        OBJECT_STORE_PATH_KIND_CHECKPOINT
    } else if path.split('/').any(|segment| segment == "_delta_log") {
        OBJECT_STORE_PATH_KIND_DELTA_LOG
    } else if path.ends_with(".parquet") {
        OBJECT_STORE_PATH_KIND_PARQUET_DATA
    } else {
        OBJECT_STORE_PATH_KIND_UNKNOWN
    }
}

fn path_kind(location: Option<&Path>) -> &'static str {
    location
        .map(classify_object_path)
        .unwrap_or(OBJECT_STORE_PATH_KIND_UNKNOWN)
}

fn path_hash(location: Option<&Path>) -> String {
    let mut hasher = DefaultHasher::new();
    location.map(Path::as_ref).unwrap_or("").hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn backend_from_url(location: &Url) -> &'static str {
    match location.scheme() {
        "file" => "local",
        "gs" => "gcs",
        "s3" | "s3a" => "s3",
        "az" | "abfs" | "abfss" => "azure",
        _ => "unknown",
    }
}

fn get_operation(options: &GetOptions) -> &'static str {
    if options.head && options.range.is_none() {
        OBJECT_STORE_OPERATION_HEAD
    } else if options.range.is_some() {
        OBJECT_STORE_OPERATION_GET_RANGE
    } else {
        OBJECT_STORE_OPERATION_GET
    }
}

pub(crate) fn get_options_range(options: &GetOptions) -> (Option<u64>, Option<u64>) {
    match options.range.as_ref() {
        Some(GetRange::Bounded(range)) => (
            Some(range.start),
            Some(range.end.saturating_sub(range.start)),
        ),
        Some(GetRange::Offset(start)) => (Some(*start), None),
        Some(GetRange::Suffix(len)) => (None, Some(*len)),
        None => (None, None),
    }
}

pub(crate) fn is_parquet_footer_candidate(location: &Path, range_len: Option<u64>) -> bool {
    classify_object_path(location) == OBJECT_STORE_PATH_KIND_PARQUET_DATA
        && range_len
            .map(|len| len <= PARQUET_FOOTER_CANDIDATE_MAX_BYTES)
            .unwrap_or(false)
}

fn object_store_error_kind(error: &ObjectStoreError) -> &'static str {
    match error {
        ObjectStoreError::Generic { .. } => "generic",
        ObjectStoreError::NotFound { .. } => "not_found",
        ObjectStoreError::InvalidPath { .. } => "invalid_path",
        ObjectStoreError::JoinError { .. } => "join_error",
        ObjectStoreError::NotSupported { .. } => "not_supported",
        ObjectStoreError::AlreadyExists { .. } => "already_exists",
        ObjectStoreError::Precondition { .. } => "precondition",
        ObjectStoreError::NotModified { .. } => "not_modified",
        ObjectStoreError::NotImplemented { .. } => "not_implemented",
        ObjectStoreError::PermissionDenied { .. } => "permission_denied",
        ObjectStoreError::Unauthenticated { .. } => "unauthenticated",
        ObjectStoreError::UnknownConfigurationKey { .. } => "unknown_configuration_key",
        _ => "unknown",
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ObjectStoreRequestTelemetry {
    backend: Arc<str>,
    operation: &'static str,
    path_kind: &'static str,
    span: Span,
    start: Instant,
}

impl ObjectStoreRequestTelemetry {
    pub(crate) fn new(
        backend: impl Into<Arc<str>>,
        operation: &'static str,
        location: Option<&Path>,
        range_start: Option<u64>,
        range_len: Option<u64>,
        cache_hit: Option<bool>,
    ) -> Self {
        let backend = backend.into();
        let path_kind = path_kind(location);
        let path_hash = path_hash(location);
        let cache_hit_value = cache_hit
            .map(|hit| hit.to_string())
            .unwrap_or_else(|| CACHE_HIT_UNKNOWN.to_string());
        let parquet_footer_candidate = location
            .map(|path| is_parquet_footer_candidate(path, range_len))
            .unwrap_or(false);

        let span = tracing::info_span!(
            OBJECT_STORE_SPAN_NAME,
            "object_store.backend" = %backend,
            "object_store.operation" = operation,
            "object_store.path_kind" = path_kind,
            "object_store.path_hash" = %path_hash,
            "object_store.range_start" = range_start.map(|value| value as i64),
            "object_store.range_len" = range_len.map(|value| value as i64),
            "object_store.cache.hit" = %cache_hit_value,
            "object_store.status" = field::Empty,
            "object_store.error.kind" = field::Empty,
            "parquet_footer_candidate" = parquet_footer_candidate,
        );

        Self {
            backend,
            operation,
            path_kind,
            span,
            start: Instant::now(),
        }
    }

    pub(crate) fn span(&self) -> Span {
        self.span.clone()
    }

    pub(crate) fn enter(&self) -> tracing::span::Entered<'_> {
        self.span.enter()
    }

    pub(crate) fn finish_success(&self, bytes: u64) {
        self.finish(STATUS_OK, None, bytes);
    }

    pub(crate) fn finish_error(&self, error: &ObjectStoreError) {
        self.finish(STATUS_ERROR, Some(object_store_error_kind(error)), 0);
    }

    fn finish_dropped(&self, bytes: u64) {
        self.finish(STATUS_DROPPED, None, bytes);
    }

    fn finish(&self, status: &'static str, error_kind: Option<&'static str>, bytes: u64) {
        self.span.record(OBJECT_STORE_STATUS_ATTR, status);
        if let Some(error_kind) = error_kind {
            self.span.record(OBJECT_STORE_ERROR_KIND_ATTR, error_kind);
        }

        let duration_ms = self.start.elapsed().as_secs_f64() * 1000.0;
        metrics::counter!(
            TRACE_OBJECT_STORE_REQUESTS_TOTAL,
            "backend" => self.backend.to_string(),
            "operation" => self.operation,
            "path_kind" => self.path_kind,
            "status" => status,
        )
        .increment(1);
        metrics::histogram!(
            TRACE_OBJECT_STORE_REQUEST_DURATION_MS,
            "backend" => self.backend.to_string(),
            "operation" => self.operation,
            "path_kind" => self.path_kind,
            "status" => status,
        )
        .record(duration_ms);

        if bytes > 0 {
            metrics::counter!(
                TRACE_OBJECT_STORE_BYTES_TOTAL,
                "backend" => self.backend.to_string(),
                "operation" => self.operation,
                "path_kind" => self.path_kind,
            )
            .increment(bytes);
        }
    }
}

pub(crate) fn observed_get_result_bytes(operation: &str, result: &GetResult) -> u64 {
    if operation == OBJECT_STORE_OPERATION_HEAD {
        0
    } else if operation == OBJECT_STORE_OPERATION_GET_RANGE {
        result.range.end.saturating_sub(result.range.start)
    } else {
        result.meta.size
    }
}

struct ObservedObjectMetaStream {
    inner: BoxStream<'static, ObjectStoreResult<ObjectMeta>>,
    telemetry: ObjectStoreRequestTelemetry,
    bytes: u64,
    complete: bool,
}

impl Stream for ObservedObjectMetaStream {
    type Item = ObjectStoreResult<ObjectMeta>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        let _entered = this.telemetry.enter();
        match this.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(meta))) => {
                this.bytes = this.bytes.saturating_add(meta.size);
                Poll::Ready(Some(Ok(meta)))
            }
            Poll::Ready(Some(Err(error))) => {
                this.complete = true;
                this.telemetry.finish_error(&error);
                Poll::Ready(Some(Err(error)))
            }
            Poll::Ready(None) => {
                this.complete = true;
                this.telemetry.finish_success(this.bytes);
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Drop for ObservedObjectMetaStream {
    fn drop(&mut self) {
        if !self.complete {
            self.telemetry.finish_dropped(self.bytes);
        }
    }
}

pub(crate) fn observe_object_meta_stream(
    stream: BoxStream<'static, ObjectStoreResult<ObjectMeta>>,
    telemetry: ObjectStoreRequestTelemetry,
) -> BoxStream<'static, ObjectStoreResult<ObjectMeta>> {
    Box::pin(ObservedObjectMetaStream {
        inner: stream,
        telemetry,
        bytes: 0,
        complete: false,
    })
}

#[derive(Debug, Clone)]
pub(crate) struct ObjectStoreSpanLayer {
    inner: ObjectStoreRef,
    backend: Arc<str>,
}

impl ObjectStoreSpanLayer {
    pub(crate) fn new(inner: ObjectStoreRef, backend: impl Into<Arc<str>>) -> Self {
        Self {
            inner,
            backend: backend.into(),
        }
    }
}

impl fmt::Display for ObjectStoreSpanLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ObjectStoreSpanLayer({}, {})", self.backend, self.inner)
    }
}

#[async_trait]
impl ObjectStore for ObjectStoreSpanLayer {
    async fn put_opts(
        &self,
        location: &Path,
        payload: PutPayload,
        opts: PutOptions,
    ) -> ObjectStoreResult<PutResult> {
        let bytes = payload.content_length() as u64;
        let telemetry = ObjectStoreRequestTelemetry::new(
            self.backend.clone(),
            OBJECT_STORE_OPERATION_PUT,
            Some(location),
            None,
            Some(bytes),
            None,
        );
        let result = self
            .inner
            .put_opts(location, payload, opts)
            .instrument(telemetry.span())
            .await;
        match &result {
            Ok(_) => telemetry.finish_success(bytes),
            Err(error) => telemetry.finish_error(error),
        }
        result
    }

    async fn put_multipart_opts(
        &self,
        location: &Path,
        opts: PutMultipartOptions,
    ) -> ObjectStoreResult<Box<dyn MultipartUpload>> {
        let telemetry = ObjectStoreRequestTelemetry::new(
            self.backend.clone(),
            OBJECT_STORE_OPERATION_PUT,
            Some(location),
            None,
            None,
            None,
        );
        let result = self
            .inner
            .put_multipart_opts(location, opts)
            .instrument(telemetry.span())
            .await;
        match &result {
            Ok(_) => telemetry.finish_success(0),
            Err(error) => telemetry.finish_error(error),
        }
        result
    }

    async fn get_opts(&self, location: &Path, options: GetOptions) -> ObjectStoreResult<GetResult> {
        let operation = get_operation(&options);
        let (range_start, range_len) = get_options_range(&options);
        let telemetry = ObjectStoreRequestTelemetry::new(
            self.backend.clone(),
            operation,
            Some(location),
            range_start,
            range_len,
            None,
        );
        let result = self
            .inner
            .get_opts(location, options)
            .instrument(telemetry.span())
            .await;
        match &result {
            Ok(result) => telemetry.finish_success(observed_get_result_bytes(operation, result)),
            Err(error) => telemetry.finish_error(error),
        }
        result
    }

    fn delete_stream(
        &self,
        locations: BoxStream<'static, ObjectStoreResult<Path>>,
    ) -> BoxStream<'static, ObjectStoreResult<Path>> {
        let backend = self.backend.clone();
        self.inner
            .delete_stream(locations)
            .map(move |result| {
                if let Ok(location) = &result {
                    let telemetry = ObjectStoreRequestTelemetry::new(
                        backend.clone(),
                        OBJECT_STORE_OPERATION_DELETE,
                        Some(location),
                        None,
                        None,
                        None,
                    );
                    let _entered = telemetry.enter();
                    telemetry.finish_success(0);
                }
                result
            })
            .boxed()
    }

    fn list(&self, prefix: Option<&Path>) -> BoxStream<'static, ObjectStoreResult<ObjectMeta>> {
        let telemetry = ObjectStoreRequestTelemetry::new(
            self.backend.clone(),
            OBJECT_STORE_OPERATION_LIST,
            prefix,
            None,
            None,
            None,
        );
        observe_object_meta_stream(self.inner.list(prefix), telemetry)
    }

    async fn list_with_delimiter(&self, prefix: Option<&Path>) -> ObjectStoreResult<ListResult> {
        let telemetry = ObjectStoreRequestTelemetry::new(
            self.backend.clone(),
            OBJECT_STORE_OPERATION_LIST_WITH_DELIMITER,
            prefix,
            None,
            None,
            None,
        );
        let result = self
            .inner
            .list_with_delimiter(prefix)
            .instrument(telemetry.span())
            .await;
        match &result {
            Ok(result) => {
                let object_bytes = result
                    .objects
                    .iter()
                    .fold(0_u64, |bytes, meta| bytes.saturating_add(meta.size));
                telemetry.finish_success(object_bytes);
            }
            Err(error) => telemetry.finish_error(error),
        }
        result
    }

    async fn copy_opts(
        &self,
        from: &Path,
        to: &Path,
        options: CopyOptions,
    ) -> ObjectStoreResult<()> {
        let telemetry = ObjectStoreRequestTelemetry::new(
            self.backend.clone(),
            OBJECT_STORE_OPERATION_COPY,
            Some(from),
            None,
            None,
            None,
        );
        let result = self
            .inner
            .copy_opts(from, to, options)
            .instrument(telemetry.span())
            .await;
        match &result {
            Ok(_) => telemetry.finish_success(0),
            Err(error) => telemetry.finish_error(error),
        }
        result
    }
}

fn object_store_with_spans(store: ObjectStoreRef, backend: &'static str) -> ObjectStoreRef {
    Arc::new(ObjectStoreSpanLayer::new(store, backend)) as ObjectStoreRef
}

pub(crate) struct PassthroughLogStoreFactory;

impl LogStoreFactory for PassthroughLogStoreFactory {
    fn with_options(
        &self,
        prefixed_store: ObjectStoreRef,
        root_store: ObjectStoreRef,
        location: &Url,
        options: &StorageConfig,
    ) -> DeltaResult<Arc<dyn LogStore>> {
        // For az:// URLs, object_store's ObjectStoreScheme::parse uses strip_bucket()
        // which assumes az://account/container/blob-path format. Scouter uses
        // az://container/blob-path (container in host, subpath in URL path).
        // strip_bucket() finds no second path segment → returns "" → delta-rs
        // applies no PrefixStore for Azure. Manually apply the correct prefix here.
        //
        // For gs://, s3://, s3a://, abfs://, abfss:// — delta-rs correctly derives
        // the subpath prefix from url.path() and applies PrefixStore via decorate_prefix.
        // Do not re-wrap those: use the already-prefixed `prefixed_store` as-is.
        let store = if location.scheme() == "az" {
            let subpath = location.path().trim_start_matches('/');
            if subpath.is_empty() {
                prefixed_store
            } else {
                let prefix = object_store::path::Path::from(subpath);
                Arc::new(object_store::prefix::PrefixStore::new(
                    root_store.clone(),
                    prefix,
                )) as ObjectStoreRef
            }
        } else {
            prefixed_store
        };
        let backend = backend_from_url(location);
        let store = object_store_with_spans(store, backend);
        let root_store = object_store_with_spans(root_store, backend);
        Ok(default_logstore(store, root_store, location, options))
    }
}

pub(crate) fn register_cloud_logstore_factories() {
    let factories = logstore_factories();
    let factory = Arc::new(PassthroughLogStoreFactory) as Arc<dyn LogStoreFactory>;
    for scheme in ["gs", "s3", "s3a", "az", "abfs", "abfss"] {
        let key = Url::parse(&format!("{}://", scheme)).expect("scheme is a valid URL prefix");
        if !factories.contains_key(&key) {
            factories.insert(key, factory.clone());
        }
    }
}

/// DataFusion 52 scalar UDF for attribute-pattern matching on `search_blob`.
///
/// `match_attr(search_blob, '%key=value%')` → `Boolean`
///
/// The pattern argument is a pre-normalized LIKE string produced by `normalize_attr_filter`:
/// it wraps the inner substring in `%...%`, so `match_attr` strips the outer `%` characters
/// and performs a `.contains(inner)` check — semantically identical to `LIKE '%inner%'`
/// but with zero regex compilation overhead and native `Utf8View` support.
///
/// **Accepted types for `search_blob` (first arg):**
/// - `Utf8View` — the canonical storage type written by `TraceSpanBatchBuilder`
/// - `Utf8` — the normalized form returned by DataFusion after some plan transformations
///
/// **Pattern (second arg):**
/// - Must always be a `Utf8` scalar literal (i.e. `lit("...")`). Array patterns are rejected.
///
/// Register once on the `SessionContext`:
/// ```rust,ignore
/// ctx.register_udf(create_attr_match_udf());
/// ```
///
/// Use in the DataFrame API via `match_attr_expr`:
/// ```rust,ignore
/// df = df.filter(match_attr_expr(col("search_blob"), lit("%svc=auth%")))?;
/// ```
/// `DynHash` (required by `ScalarUDFImpl`) is satisfied by `Hash + PartialEq + Eq`.
/// Identity is name-based — two `AttrMatchUdf` instances with the same name are equal.
#[derive(Debug)]
struct AttrMatchUdf {
    signature: Signature,
}

impl PartialEq for AttrMatchUdf {
    fn eq(&self, _other: &Self) -> bool {
        true // singleton UDF; all instances are equivalent
    }
}

impl Eq for AttrMatchUdf {}

impl std::hash::Hash for AttrMatchUdf {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name().hash(state);
    }
}

impl AttrMatchUdf {
    fn new() -> Self {
        Self {
            // Accept both Utf8View (Delta Lake read path) and Utf8 (post-cast path),
            // plus a Utf8 literal pattern. one_of covers both schema variants cleanly.
            signature: Signature::one_of(
                vec![
                    TypeSignature::Exact(vec![DataType::Utf8View, DataType::Utf8]),
                    TypeSignature::Exact(vec![DataType::Utf8, DataType::Utf8]),
                ],
                Volatility::Immutable,
            ),
        }
    }
}

impl ScalarUDFImpl for AttrMatchUdf {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn name(&self) -> &str {
        "match_attr"
    }

    fn signature(&self) -> &Signature {
        &self.signature
    }

    fn return_type(&self, _arg_types: &[DataType]) -> Result<DataType> {
        Ok(DataType::Boolean)
    }

    /// Vectorized execution: match each `search_blob` value against a constant pattern.
    ///
    /// Pattern is always a scalar literal — DataFusion folds constant expressions before
    /// dispatch, so the substring lookup is compiled exactly once per batch.
    fn invoke_with_args(&self, args: ScalarFunctionArgs) -> Result<ColumnarValue> {
        let args_slice = args.args;
        let batch_size = args.number_rows;

        // ── Pattern (second arg) — scalar literal only ───────────────────────
        let pattern_str = match &args_slice[1] {
            ColumnarValue::Scalar(ScalarValue::Utf8(Some(p)))
            | ColumnarValue::Scalar(ScalarValue::LargeUtf8(Some(p))) => p.clone(),
            _ => {
                return Err(DataFusionError::Execution(
                    "match_attr: second arg must be a non-null Utf8 scalar literal".into(),
                ));
            }
        };

        // Strip the '%...%' LIKE wrappers produced by normalize_attr_filter.
        // LIKE '%inner%'  ≡  .contains("inner")  for substring matching.
        let inner = pattern_str.trim_matches('%');

        // ── Search blob (first arg) ───────────────────────────────────────────
        match &args_slice[0] {
            // Scalar fold path — constant propagation without allocating an array.
            ColumnarValue::Scalar(s) => {
                let matched = match s {
                    ScalarValue::Utf8(Some(v))
                    | ScalarValue::LargeUtf8(Some(v))
                    | ScalarValue::Utf8View(Some(v)) => v.contains(inner),
                    _ => false,
                };
                Ok(ColumnarValue::Scalar(ScalarValue::Boolean(Some(matched))))
            }

            // Array path — vectorized substring scan.
            ColumnarValue::Array(arr) => {
                let mut builder = BooleanBuilder::with_capacity(batch_size);

                if arr.data_type() == &DataType::Utf8View {
                    // Zero-copy: StringViewArray::value() returns &str into inline or heap buffer.
                    let view_arr = arr
                        .as_any()
                        .downcast_ref::<arrow_array::StringViewArray>()
                        .ok_or_else(|| {
                            DataFusionError::Execution(
                                "match_attr: expected StringViewArray for search_blob".into(),
                            )
                        })?;
                    for i in 0..arr.len() {
                        if view_arr.is_null(i) {
                            builder.append_null();
                        } else {
                            builder.append_value(view_arr.value(i).contains(inner));
                        }
                    }
                } else {
                    // Utf8 / LargeUtf8 — normalize via Arrow cast (zero-copy reinterpret).
                    let cast_arr =
                        arrow::compute::cast(arr.as_ref(), &DataType::Utf8).map_err(|e| {
                            DataFusionError::Execution(format!(
                                "match_attr: cast to Utf8 failed: {e}"
                            ))
                        })?;
                    let str_arr =
                        cast_arr
                            .as_any()
                            .downcast_ref::<StringArray>()
                            .ok_or_else(|| {
                                DataFusionError::Execution(
                                    "match_attr: downcast to StringArray failed".into(),
                                )
                            })?;
                    for i in 0..arr.len() {
                        if str_arr.is_null(i) {
                            builder.append_null();
                        } else {
                            builder.append_value(str_arr.value(i).contains(inner));
                        }
                    }
                }

                Ok(ColumnarValue::Array(Arc::new(builder.finish())))
            }
        }
    }
}

/// Create the `match_attr` [`ScalarUDF`] using the DataFusion 52 `ScalarUDFImpl` API.
///
/// Register with a [`SessionContext`] once during initialization:
/// ```rust,ignore
/// ctx.register_udf(create_attr_match_udf());
/// ```
pub fn create_attr_match_udf() -> ScalarUDF {
    ScalarUDF::from(AttrMatchUdf::new())
}

/// Build a DataFusion [`Expr`] that calls `match_attr(search_blob, pattern)`.
///
/// Drop-in replacement for `col(blob).like(lit(pattern))` in any DataFrame
/// `.filter()`, `when()`, or aggregate context.  Handles `Utf8View` natively
/// without an intermediate cast allocation.
///
/// # Example
/// ```rust,ignore
/// // Attribute filter in a query pipeline:
/// let cond = match_attr_expr(col("search_blob"), lit("%key=value%"));
/// df = df.filter(cond)?;
///
/// // Aggregate HAVING equivalent — fold into MAX for single-pass scan:
/// let attr_agg = max(datafusion::logical_expr::cast(
///     match_attr_expr(col("search_blob"), lit("%key=value%")),
///     arrow::datatypes::DataType::Int64,
/// )).alias("attr_match");
/// ```
pub fn match_attr_expr(search_blob: Expr, pattern: Expr) -> Expr {
    create_attr_match_udf().call(vec![search_blob, pattern])
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::TryStreamExt;
    use object_store::ObjectStoreExt;
    use object_store::memory::InMemory;

    #[test]
    fn classify_object_paths_by_delta_and_parquet_kind() {
        assert_eq!(
            classify_object_path(&Path::from("traces/_delta_log/00000000000000000001.json")),
            OBJECT_STORE_PATH_KIND_DELTA_LOG
        );
        assert_eq!(
            classify_object_path(&Path::from("traces/_delta_log/_last_checkpoint")),
            OBJECT_STORE_PATH_KIND_CHECKPOINT
        );
        assert_eq!(
            classify_object_path(&Path::from(
                "traces/_delta_log/00000000000000000010.checkpoint.parquet"
            )),
            OBJECT_STORE_PATH_KIND_CHECKPOINT
        );
        assert_eq!(
            classify_object_path(&Path::from("traces/partition_date=2026-05-13/part.parquet")),
            OBJECT_STORE_PATH_KIND_PARQUET_DATA
        );
        assert_eq!(
            classify_object_path(&Path::from("traces/readme.txt")),
            OBJECT_STORE_PATH_KIND_UNKNOWN
        );
    }

    #[test]
    fn identifies_small_parquet_ranges_as_footer_candidates() {
        let path = Path::from("traces/partition_date=2026-05-13/part.parquet");

        assert!(is_parquet_footer_candidate(&path, Some(64 * 1024)));
        assert!(!is_parquet_footer_candidate(&path, Some(4 * 1024 * 1024)));
        assert!(!is_parquet_footer_candidate(
            &Path::from("traces/_delta_log/00000000000000000001.json"),
            Some(64 * 1024)
        ));
    }

    #[tokio::test]
    async fn object_store_span_layer_delegates_core_operations() {
        let inner = Arc::new(InMemory::new()) as ObjectStoreRef;
        let store = ObjectStoreSpanLayer::new(inner, "memory");
        let path = Path::from("traces/partition_date=2026-05-13/part.parquet");
        let copy_path = Path::from("traces/partition_date=2026-05-13/part-copy.parquet");

        store
            .put(&path, PutPayload::from_static(b"0123456789abcdef"))
            .await
            .unwrap();

        let meta = store.head(&path).await.unwrap();
        assert_eq!(meta.size, 16);

        let all_bytes = store.get(&path).await.unwrap().bytes().await.unwrap();
        assert_eq!(&all_bytes[..], b"0123456789abcdef");

        let range_bytes = store.get_range(&path, 4..10).await.unwrap();
        assert_eq!(&range_bytes[..], b"456789");

        let listed = store
            .list(Some(&Path::from("traces")))
            .try_collect::<Vec<_>>()
            .await
            .unwrap();
        assert_eq!(listed.len(), 1);

        let delimited = store
            .list_with_delimiter(Some(&Path::from("traces")))
            .await
            .unwrap();
        assert!(!delimited.common_prefixes.is_empty() || !delimited.objects.is_empty());

        store.copy(&path, &copy_path).await.unwrap();
        assert_eq!(store.head(&copy_path).await.unwrap().size, 16);

        store.delete(&copy_path).await.unwrap();
        assert!(store.head(&copy_path).await.is_err());
    }
}
