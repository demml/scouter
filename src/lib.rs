pub mod math;
mod scouter;
mod types;
use pyo3::prelude::*;
use scouter::_scouter::RustScouter;
use types::_types::{
    DataProfile, Distinct, DriftMap, FeatureDataProfile, FeatureDrift, FeatureMonitorProfile,
    Histogram, MonitorProfile,
};

#[pymodule]
fn _scouter(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<RustScouter>()?;
    m.add_class::<MonitorProfile>()?;
    m.add_class::<FeatureMonitorProfile>()?;
    m.add_class::<DataProfile>()?;
    m.add_class::<FeatureDataProfile>()?;
    m.add_class::<Distinct>()?;
    m.add_class::<Histogram>()?;
    m.add_class::<DriftMap>()?;
    m.add_class::<FeatureDrift>()?;
    Ok(())
}
