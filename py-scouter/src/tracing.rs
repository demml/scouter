use pyo3::prelude::*;
use scouter_tracing::tracer::*;

#[pymodule]
pub fn tracing(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<BaseTracer>()?;
    m.add_class::<ActiveSpan>()?;
    m.add_function(wrap_pyfunction!(init_tracer, m)?)?;
    //m.add_function(wrap_pyfunction!(get_tracer, m)?)?;
    Ok(())
}
