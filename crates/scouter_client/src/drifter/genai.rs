use pyo3::prelude::*;
use scouter_drift::{error::DriftError, GenAIEvaluator};
use scouter_state::app_state;
use scouter_types::genai::{GenAIDriftConfig, GenAIDriftMetric, GenAIDriftProfile};
use scouter_types::{GenAIMetricRecord, GenAIRecord};
/// Using "ClientGenAIDrifter" to avoid confusion with the server-side GenAIDrifter
pub struct ClientGenAIDrifter {}

impl Default for ClientGenAIDrifter {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientGenAIDrifter {
    pub fn new() -> Self {
        Self {}
    }

    pub fn create_drift_profile(
        &mut self,
        config: GenAIDriftConfig,
        metrics: Vec<GenAIDriftMetric>,
        workflow: Option<Bound<'_, PyAny>>,
    ) -> Result<GenAIDriftProfile, DriftError> {
        let profile = GenAIDriftProfile::new(config, metrics, workflow)?;
        Ok(profile)
    }

    pub async fn compute_drift_single(
        record: &GenAIRecord,
        profile: &GenAIDriftProfile,
    ) -> Result<Vec<GenAIMetricRecord>, DriftError> {
        let task_record = record.to_task_record(&profile.config.uid);
        let (metrics, _score, _workflow_duration) =
            GenAIEvaluator::process_drift_record(&task_record, profile).await?;
        Ok(metrics)
    }

    pub fn compute_drift(
        &mut self,
        data: Vec<GenAIRecord>,
        profile: &GenAIDriftProfile,
    ) -> Result<Vec<GenAIMetricRecord>, DriftError> {
        let results = app_state().handle().block_on(async move {
            let mut results = Vec::new();
            for record in data {
                let metrics = Self::compute_drift_single(&record, profile).await?;
                results.extend(metrics);
            }
            Ok::<_, DriftError>(results)
        })?; // propagate error from block_on

        Ok(results)
    }
}
