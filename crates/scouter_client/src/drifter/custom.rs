use scouter_error::ScouterError;
use scouter_types::custom::{CustomDriftProfile, CustomMetric, CustomMetricDriftConfig};

#[derive(Default)]
pub struct CustomDrifter {}

impl CustomDrifter {
    pub fn new() -> Self {
        Self {}
    }

    pub fn create_drift_profile(
        &mut self,
        config: CustomMetricDriftConfig,
        comparison_metrics: Vec<CustomMetric>,
        scouter_version: Option<String>,
    ) -> Result<CustomDriftProfile, ScouterError> {
        Ok(CustomDriftProfile::new(
            config,
            comparison_metrics,
            scouter_version,
        )?)
    }
}
