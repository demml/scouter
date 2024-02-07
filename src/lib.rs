mod logging;
mod math;
mod profiler;
mod types;
use math::stats::compute_array_stats;
use ndarray::Axis;
use numpy::PyReadonlyArray2;
use profiler::profiler::DataProfiler;
use pyo3::prelude::*;
use rayon::prelude::*;

#[pyfunction]
pub fn create_data_profile(array: PyReadonlyArray2<f64>) -> PyResult<()> {
    let array = array.as_array();
    let stat_vec = array
        .axis_iter(Axis(1))
        .into_par_iter()
        .map(|x| compute_array_stats(&x))
        .collect::<Vec<_>>();

    Ok(())
}

/// Python implementation for the Rusty Logger
#[pymodule]
fn _rusty_logger(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<DataProfiler>()?;
    m.add_function(wrap_pyfunction!(create_data_profile, m)?)?;
    Ok(())
}
