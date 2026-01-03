use std::sync::Arc;

use pyo3::prelude::*;
use scouter_drift::{error::DriftError, GenAIEvaluator};
use scouter_state::app_state;
use scouter_types::genai::{AssertionTask, GenAIDriftConfig, GenAIEvalProfile, LLMJudgeTask};
use scouter_types::GenAIRecord;
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
        assertions: Option<Vec<AssertionTask>>,
        llm_judge_tasks: Option<Vec<LLMJudgeTask>>,
    ) -> Result<GenAIEvalProfile, DriftError> {
        let profile = GenAIEvalProfile::new(config, assertions, llm_judge_tasks)?;
        Ok(profile)
    }

    pub async fn compute_drift_single(
        record: &GenAIRecord,
        profile: &GenAIEvalProfile,
    ) -> Result<(), DriftError> {
        let task_record = record.to_task_record(&profile.config.uid);
        let profile = Arc::new(profile.clone());
        let assertion_results =
            GenAIEvaluator::process_event_record(&task_record, &profile).await?;
        Ok(())
    }

    pub fn compute_drift(
        &mut self,
        data: Vec<GenAIRecord>,
        profile: &GenAIEvalProfile,
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
