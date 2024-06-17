mod py_scouter;
use py_scouter::_scouter::RustScouter;
use pyo3::prelude::*;
use scouter::types::_types::{
    Alert, AlertRules, AlertType, AlertZone, DataProfile, Distinct, DriftConfig, DriftMap,
    FeatureDataProfile, FeatureDrift, FeatureMonitorProfile, Histogram, MonitorConfig,
    MonitorProfile,
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
    m.add_class::<AlertRules>()?;
    m.add_class::<DriftConfig>()?;
    m.add_class::<AlertType>()?;
    m.add_class::<AlertZone>()?;
    m.add_class::<Alert>()?;
    m.add_class::<MonitorConfig>()?;
    Ok(())
}
