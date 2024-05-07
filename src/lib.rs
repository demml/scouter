mod logging;
mod math;
mod profiler;
mod types;
use profiler::_profiler::DataProfiler;
use pyo3::prelude::*;

/// Python implementation for the Rusty Logger
#[pymodule]
fn _scouter(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<DataProfiler>()?;
    Ok(())
}
