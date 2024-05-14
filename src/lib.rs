pub mod math;
mod scouter;
mod types;
use pyo3::prelude::*;
use scouter::_scouter::RustScouter;
use types::_types::{FeatureMonitorProfile, MonitorProfile};

#[pymodule]
fn _scouter(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<RustScouter>()?;
    m.add_class::<MonitorProfile>()?;
    m.add_class::<FeatureMonitorProfile>()?;
    Ok(())
}
