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
use crate::utils::BoxedSpan;
use crate::utils::{
    capture_function_arguments, format_traceback, get_context_store, get_context_var,
    get_current_active_span, get_current_context_id, set_current_span, set_function_attributes,
    set_function_type_attribute, ActiveSpanInner, FunctionType, SpanKind,
};

use chrono::{DateTime, Utc};
use opentelemetry::baggage::BaggageExt;
use opentelemetry::trace::Tracer as OTelTracer;
use opentelemetry::trace::TracerProvider;
use opentelemetry::{
    trace::{Span, SpanContext, Status, TraceContextExt, TraceState},
    Context as OtelContext, KeyValue,
};
use opentelemetry::{SpanId, TraceFlags, TraceId};
use opentelemetry_sdk::trace::SdkTracer;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::Resource;
use potato_head::create_uuid7;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};
use pyo3::IntoPyObjectExt;
use scouter_events::queue::types::TransportConfig;
use scouter_settings::http::HttpConfig;

use scouter_types::{
    is_pydantic_basemodel, pydict_to_otel_keyvalue, pyobject_to_otel_value,
    pyobject_to_tracing_json, BAGGAGE_PREFIX, EXCEPTION_TRACEBACK, SCOUTER_SCOPE,
    SCOUTER_SCOPE_DEFAULT, SCOUTER_TAG_PREFIX, SCOUTER_TRACING_INPUT, SCOUTER_TRACING_LABEL,
    SCOUTER_TRACING_OUTPUT, SPAN_ERROR, TRACE_START_TIME_KEY,
};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};
use tracing::{debug, info, instrument};

/// Global static instance of the tracer provider.
static TRACER_PROVIDER_STORE: RwLock<Option<Arc<SdkTracerProvider>>> = RwLock::new(None);
static TRACE_METADATA_STORE: OnceLock<TraceMetadataStore> = OnceLock::new();

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

    fn remove_trace(&self, trace_id: &str) -> Result<(), TraceError> {
        self.inner
            .write()
            .map_err(|e| TraceError::PoisonError(e.to_string()))?
            .remove(trace_id);
        Ok(())
    }

    fn increment_span_count(&self, trace_id: &str) -> Result<(), TraceError> {
        if let Some(mut metadata) = self.get_trace_metadata(trace_id)? {
            metadata.span_count += 1;
            self.inner
                .write()
                .map_err(|e| TraceError::PoisonError(e.to_string()))?
                .insert(trace_id.to_string(), metadata);
        }
        Ok(())
    }

    /// Decrements the span count for the given trace ID. If the span count reaches zero, the trace metadata is removed.
    fn decrement_span_count(&self, trace_id: &str) -> Result<(), TraceError> {
        if let Some(mut metadata) = self.get_trace_metadata(trace_id)? {
            if metadata.span_count > 0 {
                metadata.span_count -= 1;
            }

            if metadata.span_count == 0 {
                self.remove_trace(trace_id)?;
            } else {
                self.inner
                    .write()
                    .map_err(|e| TraceError::PoisonError(e.to_string()))?
                    .insert(trace_id.to_string(), metadata);
            }
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
#[pyfunction]
#[pyo3(signature = (
    service_name="scouter_service".to_string(),
    scope=SCOUTER_SCOPE_DEFAULT.to_string(),
    transport_config=None,
    exporter=None,
    batch_config=None,
    sample_ratio=None,
))]
#[instrument(skip_all)]
#[allow(clippy::too_many_arguments)]
pub fn init_tracer(
    py: Python,
    service_name: String,
    scope: String,
    transport_config: Option<&Bound<'_, PyAny>>,
    exporter: Option<&Bound<'_, PyAny>>,
    batch_config: Option<Py<BatchConfig>>,
    sample_ratio: Option<f64>,
) -> Result<(), TraceError> {
    debug!("Initializing tracer");
    let transport_config = match transport_config {
        Some(config) => TransportConfig::from_py_config(config)?,
        None => {
            // default to http transport config
            let config = HttpConfig::default();
            TransportConfig::Http(config)
        }
    };

    let clamped_sample_ratio = match sample_ratio {
        Some(ratio) if ratio >= 0.0 && ratio <= 1.0 => Some(ratio),
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

    let mut store_guard = TRACER_PROVIDER_STORE
        .write()
        .map_err(|e| TraceError::PoisonError(e.to_string()))?;

    if store_guard.is_some() {
        return Err(TraceError::InitializationError(
            "Tracer provider already initialized. Call shutdown_tracer() first.".to_string(),
        ));
    }

    let resource = Resource::builder()
        .with_service_name(service_name.clone())
        .with_attributes([KeyValue::new(SCOUTER_SCOPE, scope.clone())])
        .build();

    let scouter_export = ScouterSpanExporter::new(transport_config, &resource)?;

    let mut span_exporter = if let Some(exporter) = exporter {
        SpanExporterNum::from_pyobject(exporter).expect("failed to convert exporter")
    } else {
        SpanExporterNum::default()
    };

    // set the sample ratio on the exporter (this will apply to both OTLP and Scouter exporters)
    span_exporter.set_sample_ratio(clamped_sample_ratio);

    let provider = span_exporter
        .build_provider(resource, scouter_export, batch_config)
        .expect("failed to build tracer provider");

    *store_guard = Some(Arc::new(provider));

    Ok(())
}

fn reset_current_context(py: Python, token: &Py<PyAny>) -> PyResult<()> {
    let context_var = get_context_var(py)?;
    context_var.bind(py).call_method1("reset", (token,))?;
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
        debug!("Setting output on span");
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
    fn add_event(
        &self,
        py: Python,
        name: String,
        attributes: Option<Bound<'_, PyAny>>,
    ) -> Result<(), TraceError> {
        let pairs: Vec<KeyValue> = if let Some(attrs) = attributes {
            if is_pydantic_basemodel(py, &attrs)? {
                let dumped = attrs.call_method0("model_dump")?;
                let dict = dumped
                    .cast::<PyDict>()
                    .map_err(|e| TraceError::DowncastError(e.to_string()))?;
                pydict_to_otel_keyvalue(dict)?
            } else if attrs.is_instance_of::<PyDict>() {
                let dict = attrs
                    .cast::<PyDict>()
                    .map_err(|e| TraceError::DowncastError(e.to_string()))?;
                pydict_to_otel_keyvalue(dict)?
            } else {
                return Err(TraceError::EventMustBeDict);
            }
        } else {
            vec![]
        };

        self.with_inner_mut(|inner| inner.span.add_event(name, pairs))
    }

    /// Set the status of the span
    /// # Arguments
    /// * `status` - The status string ("ok", "error", or "unset")
    /// * `description` - Optional description for the status (typically used with error)
    fn set_status(&self, status: String, description: Option<String>) -> Result<(), TraceError> {
        let otel_status = match status.to_lowercase().as_str() {
            "ok" => Status::Ok,
            "error" => Status::error(description.unwrap_or_default()),
            _ => Status::Unset,
        };

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
    #[pyo3(signature = (name))]
    fn new(name: String) -> Result<Self, TraceError> {
        let tracer = get_tracer(name)?;
        Ok(BaseTracer { tracer })
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
    #[pyo3(signature = (name, kind=SpanKind::Internal, attributes=vec![], baggage=vec![], tags=vec![], label=None,  parent_context_id=None, trace_id=None, span_id=None, remote_sampled=None))]
    #[allow(clippy::too_many_arguments)]
    #[instrument(skip_all)]
    fn start_as_current_span(
        &self,
        py: Python<'_>,
        name: String,
        kind: SpanKind,
        attributes: Vec<HashMap<String, String>>,
        baggage: Vec<HashMap<String, String>>,
        tags: Vec<HashMap<String, String>>,
        label: Option<String>,
        parent_context_id: Option<String>,
        trace_id: Option<String>,
        span_id: Option<String>,
        remote_sampled: Option<bool>, // only used if both trace_id and span_id are provided (remote parent case)
    ) -> Result<ActiveSpan, TraceError> {
        // Get parent context if available
        let parent_id = parent_context_id.or_else(|| get_current_context_id(py).ok().flatten());

        // Build the base context first
        let base_ctx = if let (Some(tid), Some(sid)) = (&trace_id, &span_id) {
            // Both trace_id and span_id come from upstream service's headers
            let parsed_trace_id = TraceId::from_hex(tid)?;
            let parsed_span_id = SpanId::from_hex(sid)?; // This is the PARENT's span_id

            // Create remote context that says:
            // "My parent is span_id in trace_id, and it's from another service"
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
        } else {
            // No parent - this is a root span
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

        // Conditionally set trace_id if provided
        //if let Some(trace_id) = trace_id {
        //    let parsed_trace_id = opentelemetry::trace::TraceId::from_hex(&trace_id)
        //        .map_err(|e| TraceError::InitializationError(format!("Invalid trace_id: {}", e)))?;
        //    span_builder = span_builder.with_trace_id(parsed_trace_id);
        //}

        let mut span = BoxedSpan::new(span_builder.start_with_context(&self.tracer, &final_ctx));

        attributes.iter().for_each(|attr_map| {
            attr_map.iter().for_each(|(k, v)| {
                span.set_attribute(KeyValue::new(k.clone(), v.clone()));
            });
        });

        // set label if provided
        if let Some(label) = label {
            span.set_attribute(KeyValue::new(SCOUTER_TRACING_LABEL, label));
        }

        let context_id = Self::set_context_id(self, &mut span)?;

        let inner = Arc::new(RwLock::new(ActiveSpanInner {
            context_id,
            span,
            context_token: None,
        }));

        // set as current span
        self.set_current_span(py, &inner)?;

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
        kind=SpanKind::Internal,
        attributes=vec![],
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
        kind: SpanKind,
        attributes: Vec<HashMap<String, String>>,
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
            kind,
            attributes,
            baggage,
            tags,
            label,
            parent_context_id,
            trace_id,
            span_id,
            remote_sampled,
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

    // remove?
    pub fn shutdown(&self) -> Result<(), TraceError> {
        shutdown_tracer()
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
            Ok(provider) => provider.shutdown()?,
            Err(arc) => arc.shutdown()?,
        }
    } else {
        tracing::warn!("Tracer provider was already shut down or never initialized.");
    }

    get_trace_metadata_store().clear_all()?;
    Ok(())
}

fn get_tracer(name: String) -> Result<SdkTracer, TraceError> {
    let provider_arc = get_tracer_provider()?.ok_or_else(|| {
        TraceError::InitializationError(
            "Tracer provider not initialized or already shut down".to_string(),
        )
    })?;

    Ok(provider_arc.tracer(name))
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
    // add trace_id and span_id for easier access
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

    Ok(headers)
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
