mod py_scouter;
use crate::py_scouter::_scouter::{CustomDrifter, PsiDrifter};
use py_scouter::_scouter::{ScouterProfiler, SpcDrifter};
use pyo3::prelude::*;
use scouter::core::cron::{
    CommonCron, Every12Hours, Every15Minutes, Every1Minute, Every30Minutes, Every5Minutes,
    Every6Hours, EveryDay, EveryHour, EveryWeek,
};
use scouter::core::dispatch::types::AlertDispatchType;
use scouter::core::drift::base::{DriftType, RecordType, ServerRecord, ServerRecords};
use scouter::core::drift::custom::types::{
    AlertThreshold, CustomDriftProfile, CustomMetric, CustomMetricAlertCondition,
    CustomMetricAlertConfig, CustomMetricDriftConfig, CustomMetricServerRecord,
};
use scouter::core::drift::psi::feature_queue::PsiFeatureQueue;
use scouter::core::drift::psi::types::{
    Bin, PsiAlertConfig, PsiDriftConfig, PsiDriftMap, PsiDriftProfile, PsiFeatureDriftProfile,
    PsiServerRecord,
};
use scouter::core::drift::spc::feature_queue::SpcFeatureQueue;
use scouter::core::drift::spc::types::{
    AlertZone, SpcAlert, SpcAlertConfig, SpcAlertRule, SpcAlertType, SpcDriftConfig, SpcDriftMap,
    SpcDriftProfile, SpcFeatureAlert, SpcFeatureAlerts, SpcFeatureDrift, SpcFeatureDriftProfile,
    SpcServerRecord,
};
use scouter::core::observe::observer::{
    LatencyMetrics, ObservabilityMetrics, Observer, RouteMetrics,
};
use scouter::core::profile::types::{DataProfile, Distinct, FeatureProfile, Histogram};
use scouter::core::utils::FeatureMap;

#[pymodule]
fn _scouter(m: &Bound<'_, PyModule>) -> PyResult<()> {
    tracing_subscriber::fmt::init();

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
    m.add_class::<PsiDriftConfig>()?;
    m.add_class::<SpcFeatureAlerts>()?;
    m.add_class::<SpcFeatureAlert>()?;
    m.add_class::<SpcAlertRule>()?;
    m.add_class::<Every1Minute>()?;
    m.add_class::<Every5Minutes>()?;
    m.add_class::<Every15Minutes>()?;
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
    m.add_class::<PsiFeatureQueue>()?;
    m.add_class::<DriftType>()?;
    m.add_class::<RecordType>()?;
    m.add_class::<ServerRecords>()?;
    m.add_class::<SpcServerRecord>()?;
    m.add_class::<PsiServerRecord>()?;
    m.add_class::<ServerRecord>()?;
    m.add_class::<Observer>()?;
    m.add_class::<RouteMetrics>()?;
    m.add_class::<LatencyMetrics>()?;
    m.add_class::<ObservabilityMetrics>()?;
    m.add_class::<PsiAlertConfig>()?;
    m.add_class::<Bin>()?;
    m.add_class::<PsiFeatureDriftProfile>()?;
    m.add_class::<PsiDriftProfile>()?;
    m.add_class::<PsiDriftMap>()?;
    m.add_class::<PsiDrifter>()?;
    m.add_class::<CustomMetricAlertCondition>()?;
    m.add_class::<CustomMetricAlertConfig>()?;
    m.add_class::<CustomMetricDriftConfig>()?;
    m.add_class::<CustomMetric>()?;
    m.add_class::<AlertThreshold>()?;
    m.add_class::<CustomDriftProfile>()?;
    m.add_class::<CustomDrifter>()?;
    m.add_class::<CustomMetricServerRecord>()?;
    Ok(())
}
