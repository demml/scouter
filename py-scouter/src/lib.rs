pub mod alert;
pub mod client;
pub mod drift;
pub mod evaluate;
pub mod llm;
pub mod logging;
pub mod mock;
pub mod observe;
pub mod profile;
pub mod queue;
pub mod tracing;
pub mod types;

use pyo3::prelude::*;

use pyo3::wrap_pymodule;

#[pymodule]
fn scouter(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_wrapped(wrap_pymodule!(queue::queue))?;
    m.add_wrapped(wrap_pymodule!(logging::logging))?;
    m.add_wrapped(wrap_pymodule!(client::client))?;
    m.add_wrapped(wrap_pymodule!(drift::drift))?;
    m.add_wrapped(wrap_pymodule!(alert::alert))?;
    m.add_wrapped(wrap_pymodule!(types::types))?;
    m.add_wrapped(wrap_pymodule!(profile::profile))?;
    m.add_wrapped(wrap_pymodule!(observe::observe))?;
    m.add_wrapped(wrap_pymodule!(mock::mock))?;
    m.add_wrapped(wrap_pymodule!(llm::llm))?;
    m.add_wrapped(wrap_pymodule!(evaluate::evaluate))?;
    m.add_wrapped(wrap_pymodule!(tracing::tracing))?;

    Ok(())
}
