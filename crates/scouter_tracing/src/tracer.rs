// This file contains the core implementation logic for tracing within Scouter.
// The goal is not to re-invent the wheel here as the opentelemetry-rust crate provides a solid implementation for tracing.
// The use case we're aiming to address is users who save models, drift profiles, llm events and want to correlate them via traces/spans.
// The only way to do that in our system is to reproduce a tracer and have it be OTEL compatible so that traces are produced
// to a collector as normal, but also produced to the Scouter backend with the relevant metadata.
// This data can then be pulled inside of OpsML's UI for trace correlation and analysis.

use crate::error::TraceError;
use crate::exporter::ScouterSpanExporter;
use crate::utils::{
    FunctionType, SpanKind, capture_function_arguments, get_context_store, get_context_var, get_current_context_id, set_current_span, ActiveSpanInner, set_function_attributes, set_function_type_attribute
};
use scouter_types::records::{SCOUTER_TAG_PREFIX, SCOUTER_TRACING_INPUT, SCOUTER_TRACING_LABEL, SCOUTER_TRACING_OUTPUT, SERVICE_NAME,
 BAGGAGE_PREFIX,  TRACE_START_TIME_KEY};
use chrono::{DateTime, Utc};
use opentelemetry::baggage::BaggageExt;
use opentelemetry::trace::Tracer as OTelTracer;
use opentelemetry::Context;
use opentelemetry::{
    global::{self, BoxedSpan, BoxedTracer},
    trace::{Span,  Status, TraceContextExt},
    Context as OtelContext, KeyValue,
};
use opentelemetry_otlp::{Protocol, WithExportConfig};
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::trace::SpanProcessor;
use opentelemetry_sdk::{trace::Sampler, Resource};
use potato_head::create_uuid7;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyTuple};
use pyo3::IntoPyObjectExt;
use scouter_types::{
    is_pydantic_basemodel, pydict_to_otel_keyvalue, pyobject_to_tracing_json
};
use std::sync::RwLockWriteGuard;
use std::sync::LockResult;

use std::sync::{Arc, RwLock};
use std::time::Duration;
use std::{collections::HashMap, sync::OnceLock};

/// Global static instance of the tracer provider.
static TRACER_PROVIDER: OnceLock<SdkTracerProvider> = OnceLock::new();

// Add trace metadata store
static TRACE_METADATA_STORE: OnceLock<TraceMetadataStore> = OnceLock::new();

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
}

fn get_trace_metadata_store() -> &'static TraceMetadataStore {
    TRACE_METADATA_STORE.get_or_init(TraceMetadataStore::new)
}

// This is taken from the opentelemetry examples for adding baggage to spans
#[derive(Debug)]
struct EnrichSpanWithBaggageProcessor;

impl SpanProcessor for EnrichSpanWithBaggageProcessor {
    fn force_flush(&self) -> OTelSdkResult {
        Ok(())
    }

    fn shutdown_with_timeout(&self, _timeout: Duration) -> OTelSdkResult {
        Ok(())
    }

    fn on_start(&self, span: &mut opentelemetry_sdk::trace::Span, cx: &Context) {
        for (kk, vv) in cx.baggage().iter() {
            span.set_attribute(KeyValue::new(kk.clone(), vv.0.clone()));
        }
    }

    fn on_end(&self, _span: opentelemetry_sdk::trace::SpanData) {}
}

/// Global initialization function for the tracer.
/// This sets up the tracer provider with the specified service name, endpoint, and sampling ratio.
/// If no endpoint is provided, spans will be exported to stdout for debugging purposes.
/// # Arguments
/// * `name` - Optional service name for the tracer. Defaults to "scouter_service
/// * `endpoint` - Optional OTLP endpoint URL for exporting spans.
/// * `sample_ratio` - Optional sampling ratio between 0.0 and 1.0. Defaults to always sampling.
#[pyfunction]
#[pyo3(signature = (name=None, endpoint=None, sample_ratio=None))]
pub fn init_tracer(name: Option<String>, endpoint: Option<String>, sample_ratio: Option<f64>) {
    let name = name.unwrap_or_else(|| "scouter_service".to_string());

    TRACER_PROVIDER.get_or_init(|| {
        let resource = Resource::builder()
            .with_attributes(vec![KeyValue::new(SERVICE_NAME, name.clone())])
            .build();

        let sampler = sample_ratio
            .map(Sampler::TraceIdRatioBased)
            .unwrap_or(Sampler::AlwaysOn);

        let scouter_export = ScouterSpanExporter {
            space: "default".to_string(),
            name: name.clone(),
            version: "1.0.0".to_string(),
        };

        let provider = match endpoint {
            Some(endpoint_url) => {
                let otlp_exporter = opentelemetry_otlp::SpanExporter::builder()
                    .with_http()
                    .with_endpoint(endpoint_url)
                    .with_protocol(Protocol::HttpBinary)
                    .build()
                    .expect("Failed to create OTLP exporter");

                opentelemetry_sdk::trace::SdkTracerProvider::builder()
                    .with_span_processor(EnrichSpanWithBaggageProcessor)
                    .with_batch_exporter(otlp_exporter)
                    .with_batch_exporter(scouter_export)
                    .with_sampler(sampler)
                    .with_resource(resource)
                    .build()
            }
            None => {
                let stdout_exporter = opentelemetry_stdout::SpanExporter::default();

                opentelemetry_sdk::trace::SdkTracerProvider::builder()
                    .with_span_processor(EnrichSpanWithBaggageProcessor)
                    .with_simple_exporter(stdout_exporter)
                    .with_simple_exporter(scouter_export)
                    .with_sampler(sampler)
                    .with_resource(resource)
                    .build()
            }
        };

        global::set_tracer_provider(provider.clone());
        provider
    });
}




fn set_current_context_id(py: Python<'_>, context_id: String) -> PyResult<Py<PyAny>> {
    let context_var = get_context_var(py)?;
    Ok(context_var
        .bind(py)
        .call_method1("set", (context_id,))?
        .unbind())
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
    //context_id: String,
    //span: BoxedSpan,
    //context_token: Option<Py<PyAny>>,
    inner: Arc<RwLock<ActiveSpanInner>>,
}

#[pymethods]
impl ActiveSpan {
    #[getter]
    fn context_id(&self) -> Result<String, TraceError> {
        Ok(self.inner.read().map_err(|e| TraceError::PoisonError(e.to_string()))?.context_id.clone())
    }

    /// Set the input attribute on the span
    /// # Arguments
    /// * `input` - The input value (any Python object, but is often a dict)
    /// * `max_length` - Maximum length of the serialized input (default: 1000)
    #[pyo3(signature = (input, max_length=1000))]
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
    pub fn set_attribute(&self, key: String, value: String) -> Result<(), TraceError> {
        self.with_inner_mut(|inner| {
            inner.span.set_attribute(KeyValue::new(key, value))
        })
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
                    .downcast::<PyDict>()
                    .map_err(|e| TraceError::DowncastError(e.to_string()))?;
                pydict_to_otel_keyvalue(dict)?
            } else if attrs.is_instance_of::<PyDict>() {
                let dict = attrs
                    .downcast::<PyDict>()
                    .map_err(|e| TraceError::DowncastError(e.to_string()))?;
                pydict_to_otel_keyvalue(dict)?
            } else {
                return Err(TraceError::EventMustBeDict);
            }
        } else {
            vec![]
        };

        self.with_inner_mut(|inner| {
            inner.span.add_event(name, pairs)
        })
    }

    /// Set the status of the span
    /// # Arguments
    /// * `status` - The status string ("ok", "error", or "unset")
    /// * `description` - Optional description for the status (tyically used with error)
    fn set_status(&self, status: String, description: Option<String>) -> Result<(), TraceError> {
        let otel_status = match status.to_lowercase().as_str() {
            "ok" => Status::Ok,
            "error" => Status::error(description.unwrap_or_default()),
            _ => Status::Unset,
        };
        
        self.with_inner_mut(|inner| {
            inner.span.set_status(otel_status)
        })
    }

    /// Sync context manager enter
    fn __enter__<'py>(slf: PyRef<'py, Self>) -> PyResult<PyRef<'py, Self>> {
        Ok(slf)
    }

    /// Sync context manager exit
     #[pyo3(signature = (exc_type=None, exc_val=None, exc_tb=None))]
    fn __exit__(
        &mut self,
        py: Python<'_>,
        exc_type: Option<Py<PyAny>>,
        exc_val: Option<Py<PyAny>>,
        exc_tb: Option<Py<PyAny>>,
    ) -> Result<bool, TraceError> {

        let (context_id, trace_id, context_token) = {
            let mut inner = self.inner.write()
                .map_err(|e| TraceError::PoisonError(e.to_string()))?;
            
            // Handle exceptions and end span
            if let Some(exc_type) = exc_type {
                inner.span.set_status(Status::error("Exception occurred"));
                inner.span.set_attribute(KeyValue::new("exception.type", exc_type.to_string()));

                if let Some(exc_val) = exc_val {
                    inner.span.set_attribute(KeyValue::new("exception.value", exc_val.to_string()));
                }

                if let Some(exc_tb) = exc_tb {
                    inner.span.set_attribute(KeyValue::new("exception.traceback", exc_tb.to_string()));
                }
            }
            
            inner.span.end();
            
            // Extract values before dropping the lock
            let context_id = inner.context_id.clone();
            let trace_id = inner.span.span_context().trace_id().to_string();
            let context_token = inner.context_token.take();
            
            (context_id, trace_id, context_token)
        }; // Lock is dropped here

        // Now do operations that don't need the lock
        let store = get_trace_metadata_store();
        store.decrement_span_count(&trace_id)?;

        if let Some(token) = context_token {
            reset_current_context(py, &token)?;
        }

        get_context_store().remove(&context_id)?;
        Ok(false)
    }

    /// Async context manager enter
    fn __aenter__<'py>(
        slf: PyRef<'py, Self>,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyAny>> {
      
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
    pub fn set_attribute_static(&mut self, key: &'static str, value: String) -> Result<(), TraceError> {
        self.inner.write().map_err(|e| TraceError::PoisonError(e.to_string()))?.span.set_attribute(KeyValue::new(key, value));
        Ok(())
    }

    fn with_inner_mut<F, R>(&self, f: F) -> Result<R, TraceError>
    where
        F: FnOnce(&mut ActiveSpanInner) -> R,
    {
        let mut inner = self.inner.write()
            .map_err(|e| TraceError::PoisonError(e.to_string()))?;
        Ok(f(&mut *inner))
    }

    fn with_inner<F, R>(&self, f: F) -> Result<R, TraceError>
    where
        F: FnOnce(&ActiveSpanInner) -> R,
    {
        let inner = self.inner.read()
            .map_err(|e| TraceError::PoisonError(e.to_string()))?;
        Ok(f(&*inner))
    }
}

/// The main Tracer class
#[pyclass(subclass)]
pub struct BaseTracer {
    tracer: BoxedTracer,
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
}

#[pymethods]
impl BaseTracer {
    #[new]
    #[pyo3(signature = (name))]
    fn new(name: String) -> Self {
        let tracer = global::tracer(name.clone());
        BaseTracer { tracer }
    }

    /// Start a span and set it as the current span
    /// # Arguments
    /// * `name` - The name of the span
    /// * `kind` - Optional kind of the span ("server", "client", "
    /// producer", "consumer", "internal")
    /// * `attributes` - Optional attributes as a dictionary
    /// * `baggage` - Optional baggage items as a dictionary
    /// * `parent_context_id` - Optional parent context ID to link the span to (this is automatically set if not provided)
    #[pyo3(signature = (name, kind=SpanKind::Internal, label=None, attributes=None, baggage=None, tags=None, parent_context_id=None))]
    fn start_as_current_span(
        &self,
        py: Python<'_>,
        name: String,
        kind: SpanKind,
        label: Option<String>,
        attributes: Option<HashMap<String, String>>,
        baggage: Option<HashMap<String, String>>,
        tags: Option<HashMap<String, String>>,
        parent_context_id: Option<String>,
    ) -> Result<ActiveSpan, TraceError> {
        // Get parent context if available
        let parent_id = parent_context_id.or_else(|| get_current_context_id(py).ok().flatten());

        // Build the base context first
        let base_ctx = if let Some(parent_id) = parent_id {
            if let Some(parent_span_ctx) = get_context_store().get(&parent_id)? {
                OtelContext::current().with_remote_span_context(parent_span_ctx)
            } else {
                OtelContext::current()
            }
        } else {
            OtelContext::current()
        };


        // Need to update the context with baggage items
        let final_ctx = if let Some(baggage_items) = baggage {
            // if tags are provided, add them to baggage as well
            // these will be extracted when inserting into db
            let baggage_items = if let Some(tags) = tags {
                // need to add scouter.tag.<key> = <value> to baggage
                baggage_items.into_iter().chain(tags.into_iter().map(|(k, v)| {
                    (format!("{}.{}", SCOUTER_TAG_PREFIX, k), v)
                })).collect()
            } else {
                baggage_items
            };

            let keyvals: Vec<KeyValue> = baggage_items
                .into_iter()
                .map(|(k, v)| {
                    let baggage_key = if k == TRACE_START_TIME_KEY {
                        k // Don't prefix system keys
                    } else {
                        format!("{}.{}", BAGGAGE_PREFIX, k)
                    };
                    KeyValue::new(baggage_key, v)
                })
                .collect();

            base_ctx.with_baggage(keyvals)
        } else {
            base_ctx
        };

        // Create span with the final context (this consumes final_ctx)
        let mut span = self
            .tracer
            .span_builder(name.clone())
            .with_kind(kind.to_otel_span_kind())
            .start_with_context(&self.tracer, &final_ctx);

        // Add custom attributes
        if let Some(attrs) = attributes {
            for (key, value) in attrs {
                span.set_attribute(KeyValue::new(key, value));
            }
        }

        // set label if provided
        if let Some(label) = label {
            span.set_attribute(KeyValue::new(SCOUTER_TRACING_LABEL, label));
        }

        // if tags and 

        // Generate unique context ID and store span context
        let context_id = format!("span_{}", create_uuid7());
        let span_context = span.span_context().clone();
        Self::setup_trace_metadata(&self, &mut span)?;
        get_context_store().set(context_id.clone(), span_context)?;

        let inner = Arc::new(RwLock::new(ActiveSpanInner {
            context_id,
            span,
            context_token: None,
        }));

        // set as current span
        let py_span = Py::new(py, ActiveSpan { inner: inner.clone() })?;
        let token = set_current_span(py, py_span.bind(py).clone())?;
        inner.write().map_err(|e| TraceError::PoisonError(e.to_string()))?.context_token = Some(token);

        Ok(ActiveSpan {
            inner,
        })
    }

    #[pyo3(signature = (
        func, 
        name, 
        kind=SpanKind::Internal, 
        label=None,
        attributes=None, 
        baggage=None, 
        tags=None,
        parent_context_id=None, 
        max_length=1000, 
        func_type=FunctionType::Sync, 
        *py_args, 
        **py_kwargs
    ))]
    fn start_decorated_as_current_span<'py>(
        &self,
        py: Python<'py>,
        func: &Bound<'py, PyAny>,
        name: String,
        kind: SpanKind,
        label: Option<String>,
        attributes: Option<HashMap<String, String>>,
        baggage: Option<HashMap<String, String>>,
         tags: Option<HashMap<String, String>>,
        parent_context_id: Option<String>,
        max_length: usize,
        func_type: FunctionType,
        py_args: &Bound<'_, PyTuple>,
        py_kwargs: Option<&Bound<'_, PyDict>>,
    ) -> Result<Bound<'py, ActiveSpan>, TraceError> {
        let mut span =
            self.start_as_current_span(py, name, kind, label, attributes, baggage, tags, parent_context_id)?;

        set_function_attributes(func, &mut span)?;
        set_function_type_attribute(&func_type, &mut span)?;
        let bound_args = capture_function_arguments(py, func, py_args, py_kwargs)?;
        span.set_input(&bound_args, max_length)?;
        let py_span = Py::new(py, span)?;

        Ok(py_span.bind(py).clone())
    }
}

/// Get a tracer instance
//#[pyfunction]
//#[pyo3(signature = (name))]
//pub fn get_tracer(name: String) -> BaseTracer {
//    BaseTracer::new(name)
//}

#[pyfunction]
pub fn get_current_span_context(py: Python) -> PyResult<Option<String>> {
    get_current_context_id(py)
}

/// Helper function to force flush the tracer provider
#[pyfunction]
pub fn force_flush() -> Result<(), TraceError> {
    let provider = TRACER_PROVIDER.get().ok_or_else(|| {
        TraceError::InitializationError("Tracer provider not initialized".to_string())
    })?;
    provider.force_flush()?;
    Ok(())
}
