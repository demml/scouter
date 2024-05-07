mod logging;
mod math;
mod scouter;
mod types;
use pyo3::prelude::*;
use scouter::_scouter::Scouter;

/// Python implementation for the Rusty Logger
#[pymodule]
fn _scouter(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<Scouter>()?;
    Ok(())
}
