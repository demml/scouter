use crate::error::TraceError;
use crate::tracer::ActiveSpan;
use opentelemetry::global::BoxedSpan;
use opentelemetry::trace::SpanContext;
use opentelemetry_otlp::ExportConfig as OtlpExportConfig;
use pyo3::types::{PyDict, PyModule, PyTuple};
use pyo3::{prelude::*, IntoPyObjectExt};
use scouter_types::CompressionType;
use scouter_types::{
    FUNCTION_MODULE, FUNCTION_NAME, FUNCTION_QUALNAME, FUNCTION_STREAMING, FUNCTION_TYPE,
};
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::OnceLock;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tracing::{debug, instrument};

/// Global static instance of the context store.
static CONTEXT_STORE: OnceLock<ContextStore> = OnceLock::new();

/// Global static instance of the context variable for async context propagation. Caching the import for speed
static OTEL_CONTEXT_VAR: OnceLock<Py<PyAny>> = OnceLock::new();

// Quick access to commonly used Python modules
static PY_IMPORTS: OnceLock<HelperImports> = OnceLock::new();
const ASYNCIO_MODULE: &str = "asyncio";
const INSPECT_MODULE: &str = "inspect";
const CONTEXTVARS_MODULE: &str = "contextvars";

#[pyclass(eq)]
#[derive(PartialEq, Clone, Debug)]
pub enum FunctionType {
    Async,
    AsyncGenerator,
    SyncGenerator,
    Sync,
}

impl Display for FunctionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FunctionType::Async => write!(f, "Async"),
            FunctionType::AsyncGenerator => write!(f, "AsyncGenerator"),
            FunctionType::SyncGenerator => write!(f, "SyncGenerator"),
            FunctionType::Sync => write!(f, "Sync"),
        }
    }
}

impl FunctionType {
    pub fn as_str(&self) -> &str {
        match self {
            FunctionType::Async => "Async",
            FunctionType::AsyncGenerator => "AsyncGenerator",
            FunctionType::SyncGenerator => "SyncGenerator",
            FunctionType::Sync => "Sync",
        }
    }
}

impl std::str::FromStr for FunctionType {
    type Err = TraceError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Async" => Ok(FunctionType::Async),
            "AsyncGenerator" => Ok(FunctionType::AsyncGenerator),
            "SyncGenerator" => Ok(FunctionType::SyncGenerator),
            "Sync" => Ok(FunctionType::Sync),
            _ => Err(TraceError::InvalidFunctionType(s.to_string())),
        }
    }
}

pub struct HelperImports {
    pub asyncio: Py<PyModule>,
    pub inspect: Py<PyModule>,
}

/// Initialize and get helper imports for asyncio and inspect modules
fn get_helper_imports(py: Python<'_>) -> &'static HelperImports {
    PY_IMPORTS.get_or_init(|| {
        let asyncio = py
            .import(ASYNCIO_MODULE)
            .expect("Failed to import asyncio")
            .unbind();
        let inspect = py
            .import(INSPECT_MODULE)
            .expect("Failed to import inspect")
            .unbind();
        HelperImports { asyncio, inspect }
    })
}

fn py_asyncio(py: Python<'_>) -> &Bound<'_, PyModule> {
    let imports = get_helper_imports(py);
    imports.asyncio.bind(py)
}

fn py_inspect(py: Python<'_>) -> &Bound<'_, PyModule> {
    let imports = get_helper_imports(py);
    imports.inspect.bind(py)
}

/// Function to determine if a Python function is async, async generator, or generator
/// This is a helper util function used in tracing decorators
#[pyfunction]
pub fn get_function_type(
    py: Python<'_>,
    func: &Bound<'_, PyAny>,
) -> Result<FunctionType, TraceError> {
    // Check for async generator first (most specific)
    let is_async_gen = py_inspect(py)
        .call_method1("isasyncgenfunction", (func,))?
        .extract::<bool>()?;

    if is_async_gen {
        return Ok(FunctionType::AsyncGenerator);
    }

    // Check for sync generator
    let is_gen = py_inspect(py)
        .call_method1("isgeneratorfunction", (func,))?
        .extract::<bool>()?;

    if is_gen {
        return Ok(FunctionType::SyncGenerator);
    }

    // Check for regular async function
    let is_async = py_asyncio(py)
        .call_method1("iscoroutinefunction", (func,))?
        .extract::<bool>()?;

    if is_async {
        return Ok(FunctionType::Async);
    }

    // Default to sync
    Ok(FunctionType::Sync)
}

/// Capture function inputs by binding args and kwargs to the function signature
/// # Arguments
/// * `py` - The Python GIL token
/// * `func` - The Python function object
/// * `args` - The positional arguments passed to the function
/// * `kwargs` - The keyword arguments passed to the function
/// # Returns
/// Result with Bound arguments or TraceError
pub(crate) fn capture_function_arguments<'py>(
    py: Python<'py>,
    func: &Bound<'py, PyAny>,
    args: &Bound<'py, PyTuple>,
    kwargs: Option<&Bound<'py, PyDict>>,
) -> Result<Bound<'py, PyAny>, TraceError> {
    let sig = py_inspect(py).call_method1("signature", (func,))?;
    let bound_args = sig.call_method("bind", args, kwargs)?;
    bound_args.call_method0("apply_defaults")?;
    Ok(bound_args.getattr("arguments")?)
}

/// Set function attributes on the span
/// # Arguments
/// * `func` - The Python function object
/// * `span` - The ActiveSpan to set attributes on
/// # Returns
/// Result<(), TraceError>
#[instrument(skip_all)]
pub fn set_function_attributes(
    func: &Bound<'_, PyAny>,
    span: &mut ActiveSpan,
) -> Result<(), TraceError> {
    debug!("Setting function attributes on span");
    let function_name = match func.getattr("__name__") {
        Ok(name) => name.extract::<String>()?,
        Err(_) => "<unknown>".to_string(),
    };

    let func_module = match func.getattr("__module__") {
        Ok(module) => module.extract::<String>()?,
        Err(_) => "<unknown>".to_string(),
    };

    let func_qualname = match func.getattr("__qualname__") {
        Ok(qualname) => qualname.extract::<String>()?,
        Err(_) => "<unknown>".to_string(),
    };

    span.set_attribute_static(FUNCTION_NAME, function_name)?;
    span.set_attribute_static(FUNCTION_MODULE, func_module)?;
    span.set_attribute_static(FUNCTION_QUALNAME, func_qualname)?;

    Ok(())
}

#[instrument(skip_all)]
pub(crate) fn set_function_type_attribute(
    func_type: &FunctionType,
    span: &mut ActiveSpan,
) -> Result<(), TraceError> {
    debug!("Setting function type attribute on span");
    if func_type == &FunctionType::AsyncGenerator || func_type == &FunctionType::SyncGenerator {
        span.set_attribute_static(FUNCTION_STREAMING, "true".to_string())?;
    } else {
        span.set_attribute_static(FUNCTION_STREAMING, "false".to_string())?;
    }
    span.set_attribute_static(FUNCTION_TYPE, func_type.as_str().to_string())?;

    Ok(())
}

/// Global Context Store to hold SpanContexts associated with context IDs.
#[derive(Clone)]
pub(crate) struct ContextStore {
    inner: Arc<RwLock<HashMap<String, SpanContext>>>,
}

impl ContextStore {
    fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub(crate) fn set(&self, key: String, ctx: SpanContext) -> Result<(), TraceError> {
        self.inner
            .write()
            .map_err(|e| TraceError::PoisonError(e.to_string()))?
            .insert(key, ctx);
        Ok(())
    }

    pub(crate) fn get(&self, key: &str) -> Result<Option<SpanContext>, TraceError> {
        Ok(self
            .inner
            .read()
            .map_err(|e| TraceError::PoisonError(e.to_string()))?
            .get(key)
            .cloned())
    }

    pub(crate) fn remove(&self, key: &str) -> Result<(), TraceError> {
        self.inner
            .write()
            .map_err(|e| TraceError::PoisonError(e.to_string()))?
            .remove(key);
        Ok(())
    }
}

pub(crate) fn get_context_store() -> &'static ContextStore {
    CONTEXT_STORE.get_or_init(ContextStore::new)
}

/// Initialize the context variable for storing the current span context ID.
/// This is important for async context propagation in python.
/// This will be used to store Py<ActiveSpan> objects.
fn init_context_var(py: Python<'_>) -> PyResult<Py<PyAny>> {
    let contextvars = py.import(CONTEXTVARS_MODULE)?;
    let context_var = contextvars
        .call_method1("ContextVar", ("_otel_current_span",))?
        .unbind();
    Ok(context_var)
}

pub(crate) fn get_context_var(py: Python<'_>) -> PyResult<&Py<PyAny>> {
    Ok(OTEL_CONTEXT_VAR
        .get_or_init(|| init_context_var(py).expect("Failed to initialize context var")))
}

pub(crate) fn set_current_span(py: Python<'_>, obj: Bound<'_, ActiveSpan>) -> PyResult<Py<PyAny>> {
    let context_var = get_context_var(py)?;
    let token = context_var.bind(py).call_method1("set", (obj,))?;
    Ok(token.unbind())
}

/// Get the current context ID from the context variable.
/// Returns None if no current context is set.
pub(crate) fn get_current_context_id(py: Python<'_>) -> PyResult<Option<String>> {
    // Try to get the current value, returns None if not set
    match get_context_var(py)?.bind(py).call_method0("get") {
        Ok(val) => {
            if val.is_none() {
                Ok(None)
            } else {
                // this will be the Py<ActiveSpan> object,
                // so we need to call context_id method to get the string
                let val = val.getattr("context_id")?;
                Ok(Some(val.extract::<String>()?))
            }
        }
        Err(_) => Ok(None),
    }
}

/// Get the current active span from the context variable.
/// Returns TraceError::NoActiveSpan if no active span is set.
pub(crate) fn get_current_active_span(py: Python<'_>) -> Result<Bound<'_, PyAny>, TraceError> {
    match get_context_var(py)?.bind(py).call_method0("get") {
        Ok(val) => {
            if val.is_none() {
                Err(TraceError::NoActiveSpan)
            } else {
                Ok(val)
            }
        }
        Err(_) => Err(TraceError::NoActiveSpan),
    }
}

#[pyclass(eq)]
#[derive(PartialEq, Clone, Debug)]
pub enum SpanKind {
    Client,
    Server,
    Producer,
    Consumer,
    Internal,
}

impl Display for SpanKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpanKind::Client => write!(f, "client"),
            SpanKind::Server => write!(f, "server"),
            SpanKind::Producer => write!(f, "producer"),
            SpanKind::Consumer => write!(f, "consumer"),
            SpanKind::Internal => write!(f, "internal"),
        }
    }
}

impl SpanKind {
    pub fn to_otel_span_kind(&self) -> opentelemetry::trace::SpanKind {
        match self {
            SpanKind::Client => opentelemetry::trace::SpanKind::Client,
            SpanKind::Server => opentelemetry::trace::SpanKind::Server,
            SpanKind::Producer => opentelemetry::trace::SpanKind::Producer,
            SpanKind::Consumer => opentelemetry::trace::SpanKind::Consumer,
            SpanKind::Internal => opentelemetry::trace::SpanKind::Internal,
        }
    }
}

pub(crate) struct ActiveSpanInner {
    pub context_id: String,
    pub span: BoxedSpan,
    pub context_token: Option<Py<PyAny>>,
}

#[pyclass(eq)]
#[derive(PartialEq, Clone, Debug, Default, Serialize)]
pub enum Protocol {
    #[default]
    HttpBinary,
    HttpJson,
}

impl Protocol {
    pub fn to_otel_protocol(&self) -> opentelemetry_otlp::Protocol {
        match self {
            Protocol::HttpBinary => opentelemetry_otlp::Protocol::HttpBinary,
            Protocol::HttpJson => opentelemetry_otlp::Protocol::HttpJson,
        }
    }
}

#[derive(Debug)]
#[pyclass]
pub struct ExportConfig {
    #[pyo3(get)]
    pub endpoint: Option<String>,
    #[pyo3(get)]
    pub protocol: Protocol,
    #[pyo3(get)]
    pub timeout: Option<u64>,
}

#[pymethods]
impl ExportConfig {
    #[new]
    #[pyo3(signature = (protocol=Protocol::HttpBinary,endpoint=None,  timeout=None))]
    pub fn new(protocol: Protocol, endpoint: Option<String>, timeout: Option<u64>) -> Self {
        ExportConfig {
            endpoint,
            protocol,
            timeout,
        }
    }
}

impl ExportConfig {
    pub fn to_otel_config(&self) -> OtlpExportConfig {
        let timeout = self.timeout.map(Duration::from_secs);
        OtlpExportConfig {
            endpoint: self.endpoint.clone(),
            protocol: self.protocol.to_otel_protocol(),
            timeout,
        }
    }
}

#[derive(Debug)]
#[pyclass]
pub struct HttpConfig {
    #[pyo3(get)]
    pub headers: Option<HashMap<String, String>>,
    #[pyo3(get)]
    pub compression: Option<CompressionType>,
}

#[pymethods]
impl HttpConfig {
    #[new]
    pub fn new(
        headers: Option<HashMap<String, String>>,
        compression: Option<CompressionType>,
    ) -> Self {
        HttpConfig {
            headers,
            compression,
        }
    }
}

pub fn format_traceback(py: Python, exc_tb: &Py<PyAny>) -> Result<String, TraceError> {
    // Import the traceback module
    let traceback_module = py.import("traceback")?;

    // Use traceback.format_tb() to get a list of strings
    let tb_lines = traceback_module.call_method1("format_tb", (exc_tb.bind(py),))?;

    // Join the lines into a single string
    let empty_string = "".into_bound_py_any(py)?;
    let formatted = empty_string.call_method1("join", (tb_lines,))?;

    Ok(formatted.extract::<String>()?)
}
