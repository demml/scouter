use crate::alerts::custom::drift::CustomDrifter;
use crate::alerts::psi::drift::PsiDrifter;
use crate::alerts::spc::drift::SpcDrifter;
use scouter_sql::PostgresClient;
use chrono::NaiveDateTime;
use scouter_drift::spc::SpcFeatureAlerts;
use std::collections::BTreeMap;
use scouter_error::AlertError;

pub struct TaskAlerts {
    pub alerts: SpcFeatureAlerts,
}

impl TaskAlerts {
    pub fn new() -> Self {
        Self {
            alerts: SpcFeatureAlerts::new(false),
        }
    }
}

impl Default for TaskAlerts {
    fn default() -> Self {
        Self::new()
    }
}
