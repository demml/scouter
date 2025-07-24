use pyo3::prelude::*;
use scouter_drift::{error::DriftError, LLMEvaluator};
use scouter_types::llm::{LLMDriftConfig, LLMDriftProfile, LLMMetric};
use scouter_types::{LLMMetricRecord, LLMRecord};
use std::sync::Arc;

#[derive(Default)]
pub struct LLMDrifter {}

impl LLMDrifter {
    pub fn new() -> Self {
        Self {}
    }

    pub fn create_drift_profile(
        &mut self,
        config: LLMDriftConfig,
        metrics: Vec<LLMMetric>,
        workflow: Option<Bound<'_, PyAny>>,
    ) -> Result<LLMDriftProfile, DriftError> {
        let profile = LLMDriftProfile::new(config, metrics, workflow)?;
        Ok(profile)
    }

    pub async fn compute_drift_single(
        record: &LLMRecord,
        profile: &LLMDriftProfile,
    ) -> Result<Vec<LLMMetricRecord>, DriftError> {
        let metrics = LLMEvaluator::process_drift_record(record, profile).await?;
        Ok(metrics)
    }

    pub fn compute_drift(
        &mut self,
        data: Vec<LLMRecord>,
        profile: &LLMDriftProfile,
    ) -> Result<Vec<LLMMetricRecord>, DriftError> {
        let shared_runtime =
            Arc::new(tokio::runtime::Runtime::new().map_err(DriftError::SetupTokioRuntimeError)?);
        let mut results = Vec::new();
        for record in data {
            let metrics = shared_runtime
                .block_on(async move { Self::compute_drift_single(&record, profile).await })?;
            results.extend(metrics);
        }
        Ok(results)
    }
}
