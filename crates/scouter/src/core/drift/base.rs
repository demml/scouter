use crate::core::dispatch::types::AlertDispatchType;

// Trait for alert descriptions
// This is to be used for all kinds of feature alerts
pub trait DispatchAlertDescription {
    fn create_alert_description(&self, dispatch_type: AlertDispatchType) -> String;
}

pub struct DriftArgs {
    pub name: String,
    pub repository: String,
    pub version: String,
    pub dispatch_type: AlertDispatchType,
}

pub trait DispatchDriftConfig {
    fn get_drift_args(&self) -> DriftArgs;
}
