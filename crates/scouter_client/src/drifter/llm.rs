use pyo3::prelude::*;
use scouter_drift::error::DriftError;
use scouter_types::llm::{LLMDriftConfig, LLMDriftProfile, LLMMetric};
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
}
