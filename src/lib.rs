mod logging;
mod math;
mod scouter;
mod types;
use pyo3::prelude::*;
use scouter::_scouter::RustScouter;

#[pymodule]
fn _scouter(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<RustScouter>()?;
    Ok(())
}
