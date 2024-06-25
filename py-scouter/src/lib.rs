mod py_scouter;
use py_scouter::_scouter::{ScouterDrifter, ScouterProfiler};
use pyo3::prelude::*;
use scouter::types::_types::{
    Alert, AlertRule, AlertType, AlertZone, DataProfile, Distinct, DriftConfig, DriftMap,
    DriftProfile, FeatureAlert, FeatureAlerts, FeatureDataProfile, FeatureDrift,
    FeatureDriftProfile, Histogram, MonitorConfig, PercentageAlertRule, ProcessAlertRule,
};

#[pymodule]
fn _scouter(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<ScouterDrifter>()?;
    m.add_class::<ScouterProfiler>()?;
    m.add_class::<DriftProfile>()?;
    m.add_class::<FeatureDriftProfile>()?;
    m.add_class::<DataProfile>()?;
    m.add_class::<FeatureDataProfile>()?;
    m.add_class::<Distinct>()?;
    m.add_class::<Histogram>()?;
    m.add_class::<DriftMap>()?;
    m.add_class::<FeatureDrift>()?;
    m.add_class::<AlertRule>()?;
    m.add_class::<DriftConfig>()?;
    m.add_class::<AlertType>()?;
    m.add_class::<AlertZone>()?;
    m.add_class::<Alert>()?;
    m.add_class::<MonitorConfig>()?;
    m.add_class::<FeatureAlerts>()?;
    m.add_class::<FeatureAlert>()?;
    m.add_class::<ProcessAlertRule>()?;
    m.add_class::<PercentageAlertRule>()?;
    Ok(())
}
