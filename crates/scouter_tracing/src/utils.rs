use pyo3::prelude::*;

use crate::error::TraceError;

/// Function to determine if a Python function is async, async generator, or generator
/// This is a helper util function used in tracing decorators
#[pyfunction]
pub fn get_function_type(
    py: Python<'_>,
    func: &Bound<'_, PyAny>,
) -> Result<(bool, bool, bool), TraceError> {
    // import asyncio and inspect modules
    let asyncio = py.import("asyncio")?;
    let inspect = py.import("inspect")?;

    // check if the function is a coroutine function from asyncio
    let is_async = asyncio
        .call_method1("iscoroutinefunction", (func,))?
        .extract::<bool>()?;

    let is_async_gen = is_async
        && inspect
            .call_method1("isasyncgenfunction", (func,))?
            .extract::<bool>()?;

    let is_gen = inspect
        .call_method1("isgeneratorfunction", (func,))?
        .extract::<bool>()?;
    Ok((is_async, is_async_gen, is_gen))
}
