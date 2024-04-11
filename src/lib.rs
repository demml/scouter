mod logging;
mod math;
mod profiler;
mod types;
use math::stats::compute_array_stats;
use ndarray::Axis;
use numpy::PyReadonlyArray2;
use profiler::_profiler::DataProfiler;
use pyo3::prelude::*;
use rayon::prelude::*;

/// Python implementation for the Rusty Logger
#[pymodule]
fn _scouter(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<DataProfiler>()?;
    Ok(())
}
