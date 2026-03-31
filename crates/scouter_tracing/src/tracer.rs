// This file contains the core implementation logic for tracing within Scouter.
// The goal is not to re-invent the wheel here as the opentelemetry-rust crate provides a solid implementation for tracing.
// The use case we're aiming to address is users who save models, drift profiles, llm events and want to correlate them via traces/spans.
// The only way to do that in our system is to reproduce a tracer and have it be OTEL compatible so that traces are produced
// to a collector as normal, but also produced to the Scouter backend with the relevant metadata.
// This data can then be pulled inside of OpsML's UI for trace correlation and analysis.

use crate::error::TraceError;
use crate::exporter::processor::BatchConfig;
use crate::exporter::scouter::ScouterSpanExporter;
use crate::exporter::SpanExporterNum;
use crate::utils::py_obj_to_otel_keyvalue;
use crate::utils::BoxedSpan;
use crate::utils::{
    capture_function_arguments, format_traceback, get_context_store, get_context_var,
    get_current_active_span, get_current_context_id, parse_span_kind, parse_status,
    set_current_span, set_function_attributes, set_function_type_attribute, ActiveSpanInner,
    FunctionType, HashMapExtractor, HashMapInjector, SpanContextExt,
};

use chrono::{DateTime, Utc};
use opentelemetry::baggage::BaggageExt;
use opentelemetry::propagation::TextMapPropagator;
use opentelemetry::trace::Tracer as OTelTracer;
use opentelemetry::trace::TracerProvider;
use opentelemetry::InstrumentationScope;
use opentelemetry::{
    trace::{Span, SpanContext, Status, TraceContextExt, TraceState},
    Context as OtelContext, KeyValue,
};
use opentelemetry::{SpanId, TraceFlags, TraceId};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::SdkTracer;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::Resource;
use potato_head::create_uuid7;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};
use pyo3::IntoPyObjectExt;
use scouter_events::queue::types::TransportConfig;
use scouter_events::queue::ScouterQueue;
use scouter_settings::grpc::GrpcConfig;

use scouter_types::SCOUTER_QUEUE_RECORD;
use scouter_types::{
    pyobject_to_otel_value, pyobject_to_tracing_json, EntityType, TraceId as ScouterTraceId,
    TraceSpanRecord, BAGGAGE_PREFIX, EXCEPTION_TRACEBACK, SCOUTER_ENTITY, SCOUTER_QUEUE_EVENT,
    SCOUTER_SCOPE, SCOUTER_SCOPE_DEFAULT, SCOUTER_TAG_PREFIX, SCOUTER_TRACING_INPUT,
    SCOUTER_TRACING_LABEL, SCOUTER_TRACING_OUTPUT, SPAN_ERROR, TRACE_START_TIME_KEY,
};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::SystemTime;
use tracing::{debug, info, instrument, warn};

/// Global static instance of the tracer provider.
static TRACER_PROVIDER_STORE: RwLock<Option<Arc<SdkTracerProvider>>> = RwLock::new(None);
static TRACE_METADATA_STORE: OnceLock<TraceMetadataStore> = OnceLock::new();

// Static ScouterQueue store for global access if needed
// This allows us to set the queue anytime get_tracer is called
static SCOUTER_QUEUE_STORE: RwLock<Option<Py<ScouterQueue>>> = RwLock::new(None);

// Re-export span capture statics from scouter-types for use within this crate.
pub use scouter_types::span_capture::{CAPTURE_BUFFER, CAPTURE_BUFFER_MAX, CAPTURING};

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

/// Global initialization function for the tracer.
/// This sets up the tracer provider with the specified service name, endpoint, and sampling ratio.
/// If no endpoint is provided, spans will be exported a NoOp exporter will be used meaning
/// spans will only be sent to Scouter and not to any OTLP collector.
/// # Arguments
/// * `service_name` - Optional service name for the tracer. Defaults to "scouter_service
/// * `scope` - Optional scope for the tracer. Defaults to "scouter.tracer.{version}"
/// * `transport_config` - Optional transport configuration for the Scouter exporter
/// * `exporter` - Optional span exporter to use if you want to export spans to an OTLP collector
/// * `batch_config` - Optional batch configuration for span exporting
/// * `sample_ratio` - Optional sampling ratio between 0.0 and 1.0
/// * `scouter_queue` - Optional ScouterQueue to associate with the tracer for span queueing
/// * `schema_url` - Optional schema URL for the tracer's instrumentation scope
/// * `scope_attributes` - Optional attributes to set on the tracer's instrumentation scope
/// * `default_attributes` - Optional attributes to set on every span created by this tracer
#[pyfunction]
#[pyo3(signature = (
    service_name="scouter_service".to_string(),
    scope=SCOUTER_SCOPE_DEFAULT.to_string(),
    transport_config=None,
    exporter=None,
    batch_config=None,
    sample_ratio=None,
    scouter_queue=None,
    schema_url=None,
    scope_attributes=None,
    default_attributes=None,
))]
#[instrument(skip_all)]
#[allow(clippy::too_many_arguments)]
pub fn init_tracer(
    py: Python,
    service_name: Option<String>,
    scope: Option<String>,
    transport_config: Option<&Bound<'_, PyAny>>,
    exporter: Option<&Bound<'_, PyAny>>,
    batch_config: Option<Py<BatchConfig>>,
    sample_ratio: Option<f64>,
    scouter_queue: Option<Py<ScouterQueue>>,
    schema_url: Option<String>,
    scope_attributes: Option<Bound<'_, PyAny>>,
    default_attributes: Option<Bound<'_, PyAny>>,
) -> Result<BaseTracer, TraceError> {
    debug!("Initializing tracer");

    let service_name = service_name.unwrap_or_else(|| "scouter_service".to_string());
    let scope = scope.unwrap_or_else(|| SCOUTER_SCOPE_DEFAULT.to_string());

    let provider_exists = {
        let store_guard = TRACER_PROVIDER_STORE
            .read()
            .map_err(|e| TraceError::PoisonError(e.to_string()))?;
        store_guard.is_some()
    };

    if !provider_exists {
        debug!("Setting up tracer provider store");

        let transport_config = match transport_config {
            Some(config) => TransportConfig::from_py_config(config)?,
            None => {
                let config = GrpcConfig::default();
                TransportConfig::Grpc(config)
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

        let resource = Resource::builder()
            .with_service_name(service_name.clone())
            .with_attributes([KeyValue::new(SCOUTER_SCOPE, scope.clone())])
            .build();

        // Primary exporter for sending spans to Scouter backend
        let scouter_export = ScouterSpanExporter::new(transport_config, &resource)?;

        // Optional secondary exporter for sending spans to OTLP collector
        let mut span_exporter = if let Some(exporter) = exporter {
            SpanExporterNum::from_pyobject(exporter).expect("failed to convert exporter")
        } else {
            SpanExporterNum::default()
        };

        span_exporter.set_sample_ratio(clamped_sample_ratio);

        let provider = span_exporter
            .build_provider(resource, scouter_export, batch_config)
            .expect("failed to build tracer provider");

        let mut store_guard = TRACER_PROVIDER_STORE
            .write()
            .map_err(|e| TraceError::PoisonError(e.to_string()))?;

        if store_guard.is_none() {
            *store_guard = Some(Arc::new(provider));
        }
    } else {
        debug!("Tracer provider already initialized, skipping setup");
    }

    BaseTracer::new(
        py,
        service_name,
        schema_url,
        default_attributes,
        scope_attributes,
        scouter_queue,
    )
}

fn reset_current_context(py: Python, token: &Py<PyAny>) -> PyResult<()> {
    let context_var = get_context_var(py)?;
    context_var.bind(py).call_method1("reset", (token,))?;
    Ok(())
}

/// Helper function to create an entity event on a span when adding an item to a queue.
/// This allows us to correlate spans with queue items in the Scouter backend.
/// # Arguments
/// * `queue_item` - The item being added to the queue (must have an entity
///   type attribute for correlation)
/// * `inner` - The inner span data to add the event to
fn add_entity_event_to_span(
    queue_item: &Bound<'_, PyAny>,
    queue_bus: &Bound<'_, PyAny>,
    inner: &mut ActiveSpanInner,
) -> Result<(), TraceError> {
    let mut attributes = vec![];

    if let Ok(record_uid) = queue_item.getattr("uid") {
        let entity_type_py = queue_item.getattr("entity_type")?;
        let entity_uid = queue_bus.getattr("entity_uid")?.str()?.to_string();
        let entity_type = entity_type_py.extract::<EntityType>()?;
        let record_uid_str = record_uid.str()?.to_string();

        // set attributes for events
        attributes.push(KeyValue::new(SCOUTER_QUEUE_RECORD, record_uid_str.clone()));
        attributes.push(KeyValue::new("entity", entity_type));
        attributes.push(KeyValue::new(SCOUTER_ENTITY, entity_uid.clone()));
        inner.span.add_event(SCOUTER_QUEUE_EVENT, attributes);
    };

    Ok(())
}

/// ActiveSpan where all the magic happens
/// The active Span attempts to maintain compatibility with the OpenTelemetry Span API
#[pyclass]
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

    /// Set the entity associated with this span for correlation in Scouter
    /// # Arguments
    /// * `entity_id` - The ID of the entity to associate with this span (e.g. a record UID)
    pub fn set_entity(&self, entity_id: String) -> Result<(), TraceError> {
        self.with_inner_mut(|inner| {
            inner
                .span
                .set_attribute(KeyValue::new(SCOUTER_ENTITY, entity_id))
        })
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

    /// Add an entity into the Scouter queue associated with this span
    /// # Arguments
    /// * `alias` - The queue alias to add into
    /// * `item` - The item to add
    /// # Returns
    /// * `Result<(), TraceError>` - Ok if successful, Err otherwise
    fn add_queue_item(
        &self,
        py: Python<'_>,
        alias: String,
        item: &Bound<'_, PyAny>,
    ) -> Result<(), TraceError> {
        // check if sampling allows for this span to be sent
        debug!(
            "Attempting to add item to queue '{}' for span {}",
            alias,
            self.context_id()?
        );
        self.with_inner_mut(
            |inner| match &inner.span.span_context().trace_flags().is_sampled() {
                true => {
                    if let Some(queue) = &inner.queue {
                        let bound_queue = queue.bind(py).get_item(&alias)?;

                        // insert into queue
                        bound_queue.call_method1("insert", (item,))?;

                        // add event
                        // event attributes include entity uid and queue record uid
                        // these will also be extracted in trace aggregation layer
                        add_entity_event_to_span(item, &bound_queue, inner)?;

                        Ok(())
                    } else {
                        warn!(
                            "Queue not initialized for span {}. Skipping",
                            inner.context_id
                        );
                        Ok(())
                    }
                }
                false => {
                    debug!("Span is not sampled, skipping insert into queue");
                    Ok(())
                }
            },
        )?
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
        let (context_id, trace_id, context_token) = {
            let mut inner = self
                .inner
                .write()
                .map_err(|e| TraceError::PoisonError(e.to_string()))?;

            // Handle exceptions and end span
            if let Some(exc_type) = exc_type {
                inner.span.set_status(Status::error("Exception occurred"));
                let mut error_attributes = vec![];

                error_attributes.push(KeyValue::new("exception.type", exc_type.to_string()));

                if let Some(exc_val) = exc_val {
                    error_attributes.push(KeyValue::new("exception.value", exc_val.to_string()));
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

            inner.span.end();

            // Extract values before dropping the lock
            let context_id = inner.context_id.clone();
            let trace_id = inner.span.span_context().trace_id().to_string();
            let context_token = inner.context_token.take();
            inner.queue.take();

            (context_id, trace_id, context_token)
        };

        let store = get_trace_metadata_store();
        store.decrement_span_count(&trace_id)?;

        if let Some(token) = context_token {
            reset_current_context(py, &token)?;
        }

        get_context_store().remove(&context_id)?;
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

    fn end(&self, end_time: Option<i64>) -> Result<(), TraceError> {
        self.with_inner_mut(|inner| {
            if let Some(ts) = end_time {
                let system_time =
                    SystemTime::UNIX_EPOCH + std::time::Duration::from_nanos(ts as u64);
                inner.span.end_with_timestamp(system_time);
            } else {
                inner.span.end();
            }
        })
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

        let otel_trace = py.import("opentelemetry.trace")?;
        let trace_flags_cls = otel_trace.getattr("TraceFlags")?;
        let trace_state_cls = otel_trace.getattr("TraceState")?;
        let span_ctx_cls = otel_trace.getattr("SpanContext")?;

        let py_trace_flags = trace_flags_cls.call1((trace_flags_u8,))?;
        let py_trace_state = trace_state_cls.call0()?;

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

        if let Ok(tb_py) = exception.getattr("__traceback__") {
            if !tb_py.is_none() {
                let tb_unbound: Py<PyAny> = tb_py.unbind();
                if let Ok(stacktrace) = format_traceback(py, &tb_unbound) {
                    event_attrs.push(KeyValue::new("exception.stacktrace", stacktrace));
                }
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
}

#[pymethods]
impl BaseTracer {
    #[new]
    #[pyo3(signature = (
    name,
    schema_url=None,
    // default span attributes that are applied to every span created by this tracer
    default_attributes=None,
    // scope attributes that are applied to the tracer's instrumentation scope
    scope_attributes=None,
    queue=None,
))]
    #[instrument(skip_all)]
    fn new(
        py: Python<'_>,
        name: String,
        schema_url: Option<String>,
        default_attributes: Option<Bound<'_, PyAny>>,
        scope_attributes: Option<Bound<'_, PyAny>>,
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

        let mut scope_builder =
            InstrumentationScope::builder(name).with_version(SCOUTER_SCOPE_DEFAULT);

        if let Some(url) = schema_url {
            scope_builder = scope_builder.with_schema_url(url);
        }

        if !scope_attributes.is_empty() {
            scope_builder = scope_builder.with_attributes(scope_attributes);
        }

        let scope = scope_builder.build();
        let tracer = get_tracer(scope)?;

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
        // this mainly applies to genai evaluations as each insert checks if an eval record should be sampled
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
        } else if let Some(parent_id) = parent_id {
            // Use local parent (within same Python process)
            get_context_store()
                .get(&parent_id)?
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

        // convert baggage items to vec of KeyValue
        let baggage_items = Self::create_baggage_items(&baggage, &tags);

        let final_ctx = if !baggage_items.is_empty() {
            base_ctx.with_baggage(baggage_items)
        } else {
            base_ctx
        };

        let span_builder = self
            .tracer
            .span_builder(name.clone())
            .with_kind(kind.to_otel_span_kind());

        let mut span = BoxedSpan::new(span_builder.start_with_context(&self.tracer, &final_ctx));

        // Apply attributes — accepts both a flat dict and a list-of-dicts for OTel compat
        if let Some(attrs) = attributes {
            let kvs = py_obj_to_otel_keyvalue(py, Some(attrs.clone()))?;
            kvs.into_iter().for_each(|kv| span.set_attribute(kv));
        }

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
            span,
            context_token: None,
            queue: self.queue.as_ref().map(|q| q.clone_ref(py)),
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
        let parent_id = parent_context_id.or_else(|| get_current_context_id(py).ok().flatten());

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
        } else if let Some(parent_id) = parent_id {
            get_context_store()
                .get(&parent_id)?
                .map(|parent_ctx| OtelContext::current().with_remote_span_context(parent_ctx))
                .unwrap_or_else(OtelContext::current)
        } else if let Some(sc) = get_otel_global_span_context(py) {
            OtelContext::current().with_remote_span_context(sc)
        } else {
            OtelContext::current()
        };

        let baggage_items = Self::create_baggage_items(&baggage, &tags);
        let final_ctx = if !baggage_items.is_empty() {
            base_ctx.with_baggage(baggage_items)
        } else {
            base_ctx
        };

        let span_builder = self
            .tracer
            .span_builder(name)
            .with_kind(kind.to_otel_span_kind());

        let mut span = BoxedSpan::new(span_builder.start_with_context(&self.tracer, &final_ctx));

        if let Some(attrs) = attributes {
            let kvs = py_obj_to_otel_keyvalue(py, Some(attrs.clone()))?;
            kvs.into_iter().for_each(|kv| span.set_attribute(kv));
        }

        self.default_attributes.iter().for_each(|kv| {
            span.set_attribute(kv.clone());
        });

        if let Some(label) = label {
            span.set_attribute(KeyValue::new(SCOUTER_TRACING_LABEL, label));
        }

        let context_id = Self::set_context_id(self, &mut span)?;

        let inner = Arc::new(RwLock::new(ActiveSpanInner {
            context_id,
            span,
            context_token: None, // not pushed onto the context stack
            queue: self.queue.as_ref().map(|q| q.clone_ref(py)),
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

    /// Enable in-process span capture mode. While active, all exported spans are
    /// written to the in-memory buffer instead of being forwarded to the transport.
    pub fn enable_local_capture(&self) -> Result<(), TraceError> {
        enable_capture_impl()
    }

    /// Disable in-process span capture mode, discarding any buffered spans.
    pub fn disable_local_capture(&self) -> Result<(), TraceError> {
        disable_capture_impl()
    }

    /// Drain and return all locally captured spans, clearing the buffer.
    /// Safe to call regardless of whether capture is currently enabled.
    pub fn drain_local_spans(&self) -> Result<Vec<TraceSpanRecord>, TraceError> {
        drain_spans_impl()
    }

    /// Return captured spans matching the given trace IDs without draining the buffer.
    /// Invalid hex trace IDs are skipped with a warning.
    pub fn get_local_spans_by_trace_ids(
        &self,
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
        get_spans_by_trace_ids(&set)
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

    CAPTURING.store(false, Ordering::Release);
    CAPTURE_BUFFER
        .write()
        .unwrap_or_else(|p| p.into_inner())
        .clear();

    Ok(())
}

// ── Local span capture helpers ─────────────────────────────────────────────

fn enable_capture_impl() -> Result<(), TraceError> {
    if CAPTURING.load(Ordering::Acquire) {
        return Ok(());
    }
    let mut buf = CAPTURE_BUFFER.write().unwrap_or_else(|p| p.into_inner());
    buf.clear();
    CAPTURING.store(true, Ordering::Release);
    info!("Local span capture enabled — spans will be buffered in-process");
    Ok(())
}

fn disable_capture_impl() -> Result<(), TraceError> {
    let mut buf = CAPTURE_BUFFER.write().unwrap_or_else(|p| p.into_inner());
    if !buf.is_empty() {
        warn!(
            "disable_local_capture: discarding {} buffered spans",
            buf.len()
        );
    }
    CAPTURING.store(false, Ordering::Release);
    buf.clear();
    Ok(())
}

pub fn drain_spans_impl() -> Result<Vec<TraceSpanRecord>, TraceError> {
    Ok(scouter_types::span_capture::drain_captured_spans())
}

#[pyfunction]
pub fn enable_local_span_capture() -> Result<(), TraceError> {
    enable_capture_impl()
}

#[pyfunction]
pub fn disable_local_span_capture() -> Result<(), TraceError> {
    disable_capture_impl()
}

#[pyfunction]
pub fn drain_local_span_capture() -> Result<Vec<TraceSpanRecord>, TraceError> {
    drain_spans_impl()
}

/// Returns clones of spans matching the given trace_ids.
/// Does NOT drain the buffer — call drain_spans_impl() after all evaluations.
pub fn get_spans_by_trace_ids(
    trace_ids: &HashSet<ScouterTraceId>,
) -> Result<Vec<TraceSpanRecord>, TraceError> {
    Ok(scouter_types::span_capture::get_captured_spans_by_trace_ids(trace_ids))
}

/// Returns a clone of all captured spans without draining.
pub fn get_all_captured_spans() -> Result<Vec<TraceSpanRecord>, TraceError> {
    Ok(scouter_types::span_capture::get_all_captured_spans())
}

fn get_tracer(scope: InstrumentationScope) -> Result<SdkTracer, TraceError> {
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

    let trace_id_int: u128 = span_ctx.getattr("trace_id").ok()?.extract().ok()?;
    let span_id_int: u64 = span_ctx.getattr("span_id").ok()?.extract().ok()?;
    let flags_int: u8 = span_ctx.getattr("trace_flags").ok()?.extract().ok()?;

    let sc = SpanContext::new(
        TraceId::from(trace_id_int),
        SpanId::from(span_id_int),
        TraceFlags::new(flags_int),
        false, // not remote — same process
        TraceState::default(),
    );
    if sc.is_valid() {
        Some(sc)
    } else {
        None
    }
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

    let trace_id_int: u128 = span_ctx.getattr("trace_id").ok()?.extract().ok()?;
    let span_id_int: u64 = span_ctx.getattr("span_id").ok()?.extract().ok()?;
    let flags_int: u8 = span_ctx.getattr("trace_flags").ok()?.extract().ok()?;

    let sc = SpanContext::new(
        TraceId::from(trace_id_int),
        SpanId::from(span_id_int),
        TraceFlags::new(flags_int),
        true, // is_remote — came from wire headers
        TraceState::default(),
    );
    if sc.is_valid() {
        Some(sc)
    } else {
        None
    }
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
mod capture_tests {
    use super::*;

    fn reset() {
        CAPTURING.store(false, Ordering::Release);
        CAPTURE_BUFFER.write().unwrap().clear();
    }

    #[test]
    fn test_enable_sets_capturing_true() {
        reset();
        enable_capture_impl().unwrap();
        assert!(CAPTURING.load(Ordering::Acquire));
        reset();
    }

    #[test]
    fn test_disable_sets_capturing_false() {
        reset();
        CAPTURING.store(true, Ordering::Release);
        disable_capture_impl().unwrap();
        assert!(!CAPTURING.load(Ordering::Acquire));
        reset();
    }

    #[test]
    fn test_drain_clears_and_returns_empty() {
        reset();
        enable_capture_impl().unwrap();
        let drained = drain_spans_impl().unwrap();
        assert!(drained.is_empty());
        assert!(CAPTURING.load(Ordering::Acquire)); // still capturing after drain
        reset();
    }

    #[test]
    fn test_drain_returns_empty_when_capture_off() {
        reset();
        let result = drain_spans_impl().unwrap();
        assert!(result.is_empty());
        reset();
    }

    #[test]
    fn test_drain_returns_and_clears_populated_buffer() {
        reset();
        enable_capture_impl().unwrap();
        CAPTURE_BUFFER
            .write()
            .unwrap()
            .push(TraceSpanRecord::default());
        let drained = drain_spans_impl().unwrap();
        assert_eq!(drained.len(), 1);
        assert!(CAPTURE_BUFFER.read().unwrap().is_empty());
        reset();
    }

    #[test]
    fn test_enable_clears_existing_buffer() {
        reset();
        CAPTURE_BUFFER
            .write()
            .unwrap()
            .push(TraceSpanRecord::default());
        enable_capture_impl().unwrap();
        assert!(CAPTURE_BUFFER.read().unwrap().is_empty());
        reset();
    }

    #[test]
    fn test_get_spans_by_trace_ids_filters_without_drain() {
        reset();
        enable_capture_impl().unwrap();
        let id_a = ScouterTraceId::from_bytes([1u8; 16]);
        let id_b = ScouterTraceId::from_bytes([2u8; 16]);
        let span_a = TraceSpanRecord {
            trace_id: id_a,
            ..TraceSpanRecord::default()
        };
        let span_b = TraceSpanRecord {
            trace_id: id_b,
            ..TraceSpanRecord::default()
        };
        CAPTURE_BUFFER.write().unwrap().extend([span_a, span_b]);
        let set = HashSet::from([id_a]);
        let result = get_spans_by_trace_ids(&set).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].trace_id, id_a);
        // buffer not drained
        assert_eq!(CAPTURE_BUFFER.read().unwrap().len(), 2);
        reset();
    }

    #[test]
    fn test_buffer_overflow_drops_excess() {
        reset();
        enable_capture_impl().unwrap();
        {
            let mut buf = CAPTURE_BUFFER.write().unwrap();
            for _ in 0..CAPTURE_BUFFER_MAX {
                buf.push(TraceSpanRecord::default());
            }
        }
        let available = CAPTURE_BUFFER_MAX.saturating_sub(CAPTURE_BUFFER.read().unwrap().len());
        assert_eq!(available, 0);
        // Buffer at max, no panic
        assert_eq!(CAPTURE_BUFFER.read().unwrap().len(), CAPTURE_BUFFER_MAX);
        reset();
    }

    #[test]
    fn test_drain_returns_spans_even_when_not_capturing() {
        reset();
        CAPTURE_BUFFER
            .write()
            .unwrap()
            .push(TraceSpanRecord::default());
        let result = drain_spans_impl().unwrap();
        assert_eq!(result.len(), 1); // drain is unconditional
        assert!(CAPTURE_BUFFER.read().unwrap().is_empty());
        reset();
    }
}
