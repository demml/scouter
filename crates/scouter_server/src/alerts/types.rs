use crate::alerts::custom::drift::CustomDrifter;
use crate::alerts::psi::drift::PsiDrifter;
use crate::alerts::spc::drift::SpcDrifter;
use crate::sql::postgres::PostgresClient;
use chrono::NaiveDateTime;
use scouter::core::drift::spc::types::SpcFeatureAlerts;
use std::collections::BTreeMap;

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

#[allow(clippy::enum_variant_names)]
pub enum Drifter {
    SpcDrifter(SpcDrifter),
    PsiDrifter(PsiDrifter),
    CustomDrifter(CustomDrifter),
}

impl Drifter {
    pub async fn check_for_alerts(
        &self,
        db_client: &PostgresClient,
        previous_run: NaiveDateTime,
    ) -> Result<Option<Vec<BTreeMap<String, String>>>, anyhow::Error> {
        match self {
            Drifter::SpcDrifter(drifter) => drifter.check_for_alerts(db_client, previous_run).await,
            Drifter::PsiDrifter(drifter) => drifter.check_for_alerts(db_client, previous_run).await,
            Drifter::CustomDrifter(drifter) => {
                drifter.check_for_alerts(db_client, previous_run).await
            }
        }
    }
}
