// This file contains the core implementation logic for tracing within Scouter.
// The goal is not to re-invent the wheel here as the opentelemetry-rust crate provides a solid foundation for tracing.
// The use case we're aiming to address is users who save models, drift profiles, llm events and what to correlate them via traces/spans.
// The only way to do that in our system is to reproduce a tracer and have it be OTEL compatible so that traces are produced
// to a collector as normal, but also produced to the Scouter backend with the relevant metadata.
// This data can then be pulled inside of OpsML's UI for trace correlation and analysis.

use crate::error::TraceError;
use chrono::{DateTime, Utc};
use opentelemetry::baggage::BaggageExt;
use opentelemetry::trace::Tracer as OTelTracer;
use opentelemetry::Context;
use opentelemetry::{
    global::{self, BoxedSpan, BoxedTracer},
    trace::{Span, SpanContext, SpanKind, Status, TraceContextExt},
    Context as OtelContext, KeyValue,
};
use opentelemetry_otlp::{Protocol, WithExportConfig};
use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::trace::SdkTracerProvider;
use opentelemetry_sdk::trace::SpanProcessor;
use opentelemetry_sdk::{trace::Sampler, Resource};
use potato_head::create_uuid7;
use pyo3::prelude::*;
use pyo3::IntoPyObjectExt;
use scouter_types::{is_pydantic_basemodel, pydantic_to_value, pyobject_to_json, BAGGAGE_PREFIX};
use serde_json::Value;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use std::{collections::HashMap, sync::OnceLock};

/// Global static instance of the tracer provider.
static TRACER_PROVIDER: OnceLock<SdkTracerProvider> = OnceLock::new();

/// Global static instance of the context store.
static CONTEXT_STORE: OnceLock<ContextStore> = OnceLock::new();

/// Global static instance of the context variable for async context propagation. Caching the import for speed
static CONTEXT_VAR: OnceLock<Py<PyAny>> = OnceLock::new();

const TRACE_START_TIME_KEY: &str = "scouter.trace.start_time";

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
            .with_attributes(vec![KeyValue::new("service.name", name.clone())])
            .build();

        let sampler = sample_ratio
            .map(Sampler::TraceIdRatioBased)
            .unwrap_or(Sampler::AlwaysOn);

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
                    .with_sampler(sampler)
                    .with_resource(resource)
                    .build()
            }
            None => {
                let stdout_exporter = opentelemetry_stdout::SpanExporter::default();

                opentelemetry_sdk::trace::SdkTracerProvider::builder()
                    .with_span_processor(EnrichSpanWithBaggageProcessor)
                    .with_simple_exporter(stdout_exporter)
                    .with_sampler(sampler)
                    .with_resource(resource)
                    .build()
            }
        };

        global::set_tracer_provider(provider.clone());
        provider
    });
}

/// Global Context Store to hold SpanContexts associated with context IDs.
#[derive(Clone)]
struct ContextStore {
    inner: Arc<RwLock<HashMap<String, SpanContext>>>,
}

impl ContextStore {
    fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn set(&self, key: String, ctx: SpanContext) -> Result<(), TraceError> {
        self.inner
            .write()
            .map_err(|e| TraceError::PoisonError(e.to_string()))?
            .insert(key, ctx);
        Ok(())
    }

    fn get(&self, key: &str) -> Result<Option<SpanContext>, TraceError> {
        Ok(self
            .inner
            .read()
            .map_err(|e| TraceError::PoisonError(e.to_string()))?
            .get(key)
            .cloned())
    }

    fn remove(&self, key: &str) -> Result<(), TraceError> {
        self.inner
            .write()
            .map_err(|e| TraceError::PoisonError(e.to_string()))?
            .remove(key);
        Ok(())
    }
}

fn get_context_store() -> &'static ContextStore {
    CONTEXT_STORE.get_or_init(ContextStore::new)
}

/// Initialize the context variable for storing the current span context ID.
/// This is important for async context propagation in python.
fn init_context_var(py: Python<'_>) -> PyResult<Py<PyAny>> {
    let contextvars = py.import("contextvars")?;
    let context_var = contextvars
        .call_method1("ContextVar", ("_otel_current_span",))?
        .unbind();
    Ok(context_var)
}

fn get_context_var(py: Python<'_>) -> PyResult<&Py<PyAny>> {
    Ok(CONTEXT_VAR.get_or_init(|| init_context_var(py).expect("Failed to initialize context var")))
}

fn get_current_context_id(py: Python<'_>) -> PyResult<Option<String>> {
    let context_var = get_context_var(py)?;

    // Try to get the current value, returns None if not set
    match context_var.bind(py).call_method0("get") {
        Ok(val) => {
            if val.is_none() {
                Ok(None)
            } else {
                Ok(Some(val.extract::<String>()?))
            }
        }
        Err(_) => Ok(None),
    }
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
    context_id: String,
    span: Option<BoxedSpan>,
    context_token: Option<Py<PyAny>>,
}

#[pymethods]
impl ActiveSpan {
    #[getter]
    fn context_id(&self) -> String {
        self.context_id.clone()
    }

    /// Set an attribute on the span
    /// # Arguments
    /// * `key` - The attribute key
    /// * `value` - The attribute value
    fn set_attribute(&mut self, key: String, value: String) -> PyResult<()> {
        if let Some(ref mut span) = self.span {
            span.set_attribute(KeyValue::new(key, value));
        }
        Ok(())
    }

    /// Add an event to the span
    /// # Arguments
    /// * `name` - The event name
    /// * `attributes` - The event attributes as a dictionary or pydantic BaseModel
    fn add_event(
        &mut self,
        py: Python,
        name: String,
        attributes: Bound<'_, PyAny>,
    ) -> PyResult<()> {
        if let Some(ref mut span) = self.span {
            let attributes_val = attributes_to_value(py, &attributes)?;
            let otel_val =
                opentelemetry::Value::from(serde_json::to_string(&attributes_val).unwrap());
            span.add_event(name, vec![KeyValue::new("attributes", otel_val)]);
        }
        Ok(())
    }

    /// Set the status of the span
    /// # Arguments
    /// * `status` - The status string ("ok", "error", or "unset")
    /// * `description` - Optional description for the status (tyically used with error)
    fn set_status(&mut self, status: String, description: Option<String>) -> PyResult<()> {
        if let Some(ref mut span) = self.span {
            let otel_status = match status.to_lowercase().as_str() {
                "ok" => Status::Ok,
                "error" => Status::error(description.unwrap_or_default()),
                _ => Status::Unset,
            };
            span.set_status(otel_status);
        }
        Ok(())
    }

    /// Sync context manager enter
    fn __enter__<'py>(mut slf: PyRefMut<'py, Self>) -> PyResult<PyRefMut<'py, Self>> {
        let py = slf.py();
        let token = set_current_context_id(py, slf.context_id.clone())?;
        slf.context_token = Some(token);
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
        if let Some(mut span) = self.span.take() {
            if let Some(exc_type) = exc_type {
                span.set_status(Status::error("Exception occurred"));
                span.set_attribute(KeyValue::new("exception.type", exc_type.to_string()));

                if let Some(exc_val) = exc_val {
                    span.set_attribute(KeyValue::new("exception.value", exc_val.to_string()));
                }

                if let Some(exc_tb) = exc_tb {
                    span.set_attribute(KeyValue::new("exception.traceback", exc_tb.to_string()));
                }
            }
            span.end();

            // decrement span count
            let trace_id = span.span_context().trace_id().to_string();
            let store = get_trace_metadata_store();
            store.decrement_span_count(&trace_id)?;
        }

        if let Some(token) = self.context_token.take() {
            reset_current_context(py, &token)?;
        }

        get_context_store().remove(&self.context_id)?;
        Ok(false)
    }

    /// Async context manager enter
    fn __aenter__<'py>(
        mut slf: PyRefMut<'py, Self>,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let context_id = slf.context_id.clone();
        let token = set_current_context_id(py, context_id)?;
        slf.context_token = Some(token);
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
    #[pyo3(signature = (name, kind=None, attributes=None, baggage=None, parent_context_id=None))]
    fn start_as_current_span(
        &self,
        py: Python<'_>,
        name: String,
        kind: Option<String>,
        attributes: Option<HashMap<String, String>>,
        baggage: Option<HashMap<String, String>>,
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

        // Determine span kind
        let span_kind = match kind.as_deref() {
            Some("server") => SpanKind::Server,
            Some("client") => SpanKind::Client,
            Some("producer") => SpanKind::Producer,
            Some("consumer") => SpanKind::Consumer,
            Some("internal") | _ => SpanKind::Internal,
        };

        // Need to update the context with baggage items
        let final_ctx = if let Some(baggage_items) = baggage {
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
            .with_kind(span_kind)
            .start_with_context(&self.tracer, &final_ctx);

        // Add custom attributes
        if let Some(attrs) = attributes {
            for (key, value) in attrs {
                span.set_attribute(KeyValue::new(key, value));
            }
        }

        // Generate unique context ID and store span context
        let context_id = format!("span_{}", create_uuid7());
        let span_context = span.span_context().clone();
        Self::setup_trace_metadata(&self, &mut span)?;
        get_context_store().set(context_id.clone(), span_context)?;

        Ok(ActiveSpan {
            context_id,
            span: Some(span),
            context_token: None,
        })
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

fn attributes_to_value(py: Python, obj: &Bound<'_, PyAny>) -> Result<Value, TraceError> {
    if is_pydantic_basemodel(py, obj)? {
        return Ok(pydantic_to_value(obj)?);
    }
    Ok(pyobject_to_json(obj)?)
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
