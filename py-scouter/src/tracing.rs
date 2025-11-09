use pyo3::prelude::*;
use scouter_tracing::tracer::*;
use scouter_tracing::utils::{FunctionType, SpanKind};

#[pymodule]
pub fn tracing(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<BaseTracer>()?;
    m.add_class::<ActiveSpan>()?;
    m.add_class::<SpanKind>()?;
    m.add_class::<FunctionType>()?;
    m.add_function(wrap_pyfunction!(init_tracer, m)?)?;
    //m.add_function(wrap_pyfunction!(get_tracer, m)?)?;
    Ok(())
}
