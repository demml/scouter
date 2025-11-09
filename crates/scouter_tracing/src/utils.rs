use crate::error::TraceError;
use crate::tracer::ActiveSpan;
use opentelemetry::trace::SpanContext;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule, PyTuple};
use scouter_types::records::{
    FUNCTION_MODULE, FUNCTION_NAME, FUNCTION_QUALNAME, FUNCTION_STREAMING, FUNCTION_TYPE,
};
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::OnceLock;
use std::sync::{Arc, RwLock};

/// Global static instance of the context store.
static CONTEXT_STORE: OnceLock<ContextStore> = OnceLock::new();

/// Global static instance of the context variable for async context propagation. Caching the import for speed
static CONTEXT_VAR: OnceLock<Py<PyAny>> = OnceLock::new();

// Quick access to commonly used Python modules
static PY_IMPORTS: OnceLock<HelperImports> = OnceLock::new();
const ASYNCIO_MODULE: &'static str = "asyncio";
const INSPECT_MODULE: &'static str = "inspect";
const CONTEXTVARS_MODULE: &'static str = "contextvars";

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

    pub fn from_str(s: &str) -> Self {
        match s {
            "async" => FunctionType::Async,
            "async_generator" => FunctionType::AsyncGenerator,
            "sync_generator" => FunctionType::SyncGenerator,
            "sync" => FunctionType::Sync,
            _ => FunctionType::Sync,
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
) -> Result<(bool, bool, bool), TraceError> {
    // check if the function is a coroutine function from asyncio
    let is_async = py_asyncio(py)
        .call_method1("iscoroutinefunction", (func,))?
        .extract::<bool>()?;

    let is_async_gen = is_async
        && py_inspect(py)
            .call_method1("isasyncgenfunction", (func,))?
            .extract::<bool>()?;

    let is_gen = py_inspect(py)
        .call_method1("isgeneratorfunction", (func,))?
        .extract::<bool>()?;
    Ok((is_async, is_async_gen, is_gen))
}

/// Capture function inputs by binding args and kwargs to the function signature
/// # Arguments
/// * `py` - The Python GIL token
/// * `func` - The Python function object
/// * `py_args` - The positional arguments passed to the function
/// * `py_kwargs` - The keyword arguments passed to the function
/// Returns Result with Bound arguments or TraceError
pub fn capture_function_inputs<'py>(
    py: Python<'py>,
    func: &Bound<'py, PyAny>,
    py_args: &Bound<'py, PyTuple>,
    py_kwargs: Option<&Bound<'py, PyDict>>,
) -> Result<Bound<'py, PyAny>, TraceError> {
    let sig = py_inspect(py).call_method1("signature", (func,))?;
    let bound_args = sig.call_method1("bind", (py_args, py_kwargs))?;
    bound_args.call_method0("apply_defaults")?;
    Ok(bound_args)
}

/// Set function attributes on the span
/// # Arguments
/// * `func` - The Python function object
/// * `span` - The ActiveSpan to set attributes on
/// Returns Result<(), TraceError>
pub fn set_function_attributes(
    func: &Bound<'_, PyAny>,
    span: &mut ActiveSpan,
) -> Result<(), TraceError> {
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

    span.set_attribute_static(FUNCTION_NAME, function_name);
    span.set_attribute_static(FUNCTION_MODULE, func_module);
    span.set_attribute_static(FUNCTION_QUALNAME, func_qualname);
    Ok(())
}

pub(crate) fn set_function_type_attribute(
    func_type: &FunctionType,
    span: &mut ActiveSpan,
) -> Result<(), TraceError> {
    if func_type == &FunctionType::AsyncGenerator || func_type == &FunctionType::SyncGenerator {
        span.set_attribute_static(FUNCTION_STREAMING, "true".to_string());
    } else {
        span.set_attribute_static(FUNCTION_STREAMING, "false".to_string());
    }
    span.set_attribute_static(FUNCTION_TYPE, func_type.as_str().to_string());

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
fn init_context_var(py: Python<'_>) -> PyResult<Py<PyAny>> {
    let contextvars = py.import(CONTEXTVARS_MODULE)?;
    let context_var = contextvars
        .call_method1("ContextVar", ("_otel_current_span",))?
        .unbind();
    Ok(context_var)
}

pub(crate) fn get_context_var(py: Python<'_>) -> PyResult<&Py<PyAny>> {
    Ok(CONTEXT_VAR.get_or_init(|| init_context_var(py).expect("Failed to initialize context var")))
}

pub(crate) fn get_current_context_id(py: Python<'_>) -> PyResult<Option<String>> {
    // Try to get the current value, returns None if not set
    match get_context_var(py)?.bind(py).call_method0("get") {
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
