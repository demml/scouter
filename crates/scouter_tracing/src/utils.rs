use crate::error::TraceError;
use crate::tracer::ActiveSpan;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule, PyTuple};
use std::sync::OnceLock;

// Quick access to commonly used Python modules
static PY_IMPORTS: OnceLock<HelperImports> = OnceLock::new();
const ASYNCIO_MODULE: &str = "asyncio";
const INSPECT_MODULE: &str = "inspect";

pub enum FunctionType {
    Async,
    AsyncGenerator,
    SyncGenerator,
    Sync,
}

impl FunctionType {
    pub fn as_str(&self) -> &str {
        match self {
            FunctionType::Async => "async",
            FunctionType::AsyncGenerator => "async_generator",
            FunctionType::SyncGenerator => "sync_generator",
            FunctionType::Sync => "sync",
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

    span.set_attribute("function.name".to_string(), function_name)?;
    span.set_attribute("function.module".to_string(), func_module)?;
    span.set_attribute("function.qualname".to_string(), func_qualname)?;
    Ok(())
}
