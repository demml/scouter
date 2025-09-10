use pyo3::prelude::*;
use scouter_drift::{error::DriftError, LLMEvaluator};
use scouter_types::llm::{LLMDriftConfig, LLMDriftMetric, LLMDriftProfile};
use scouter_types::{LLMMetricRecord, LLMRecord};
pub struct LLMDrifter {
    pub runtime: tokio::runtime::Runtime,
}

impl Default for LLMDrifter {
    fn default() -> Self {
        Self::new()
    }
}

impl LLMDrifter {
    pub fn new() -> Self {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        Self { runtime }
    }

    pub fn create_drift_profile(
        &mut self,
        config: LLMDriftConfig,
        metrics: Vec<LLMDriftMetric>,
        workflow: Option<Bound<'_, PyAny>>,
    ) -> Result<LLMDriftProfile, DriftError> {
        let profile = LLMDriftProfile::new(config, metrics, workflow)?;
        Ok(profile)
    }

    pub async fn compute_drift_single(
        record: &LLMRecord,
        profile: &LLMDriftProfile,
    ) -> Result<Vec<LLMMetricRecord>, DriftError> {
        let (metrics, _score, _workflow_duration) =
            LLMEvaluator::process_drift_record(record, profile).await?;
        Ok(metrics)
    }

    pub fn compute_drift(
        &mut self,
        data: Vec<LLMRecord>,
        profile: &LLMDriftProfile,
    ) -> Result<Vec<LLMMetricRecord>, DriftError> {
        let results = self.runtime.block_on(async move {
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
