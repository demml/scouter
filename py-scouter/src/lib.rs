mod py_scouter;
use py_scouter::_scouter::{ScouterProfiler, SpcDrifter};
use pyo3::prelude::*;
use scouter::core::cron::{
    CommonCron, Every12Hours, Every30Minutes, Every6Hours, EveryDay, EveryHour, EveryWeek,
};
use scouter::core::dispatch::types::AlertDispatchType;
use scouter::core::drift::base::{RecordType, ServerRecords, SpcServerRecord};
use scouter::core::drift::spc::feature_queue::SpcFeatureQueue;
use scouter::core::drift::spc::types::{
    AlertZone, FeatureMap, SpcAlert, SpcAlertConfig, SpcAlertRule, SpcAlertType, SpcDriftConfig,
    SpcDriftMap, SpcDriftProfile, SpcFeatureAlert, SpcFeatureAlerts, SpcFeatureDrift,
    SpcFeatureDriftProfile,
};
use scouter::core::profile::types::{DataProfile, Distinct, FeatureProfile, Histogram};
use scouter::core::utils::DriftType;

#[pymodule]
fn _scouter(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<SpcDrifter>()?;
    m.add_class::<ScouterProfiler>()?;
    m.add_class::<SpcDriftProfile>()?;
    m.add_class::<SpcFeatureDriftProfile>()?;
    m.add_class::<DataProfile>()?;
    m.add_class::<FeatureProfile>()?;
    m.add_class::<Distinct>()?;
    m.add_class::<Histogram>()?;
    m.add_class::<SpcDriftMap>()?;
    m.add_class::<SpcFeatureDrift>()?;
    m.add_class::<SpcDriftConfig>()?;
    m.add_class::<SpcAlertType>()?;
    m.add_class::<AlertZone>()?;
    m.add_class::<SpcAlert>()?;
    m.add_class::<SpcDriftConfig>()?;
    m.add_class::<SpcFeatureAlerts>()?;
    m.add_class::<SpcFeatureAlert>()?;
    m.add_class::<SpcAlertRule>()?;
    m.add_class::<Every30Minutes>()?;
    m.add_class::<EveryHour>()?;
    m.add_class::<Every6Hours>()?;
    m.add_class::<Every12Hours>()?;
    m.add_class::<EveryDay>()?;
    m.add_class::<EveryWeek>()?;
    m.add_class::<CommonCron>()?;
    m.add_class::<SpcAlertConfig>()?;
    m.add_class::<AlertDispatchType>()?;
    m.add_class::<FeatureMap>()?;
    m.add_class::<SpcFeatureQueue>()?;
    m.add_class::<DriftType>()?;
    m.add_class::<RecordType>()?;
    m.add_class::<ServerRecords>()?;
    m.add_class::<SpcServerRecord>()?;
    Ok(())
}
