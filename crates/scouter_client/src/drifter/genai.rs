use scouter_drift::error::DriftError;
use scouter_evaluate::evaluate::GenAIEvaluator;
use scouter_state::app_state;
use scouter_types::genai::{
    AssertionTask, GenAIDriftConfig, GenAIEvalProfile, GenAIEvalSet, LLMJudgeTask,
};
use scouter_types::GenAIEvalRecord;
use std::sync::Arc;
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
        assertions_tasks: Option<Vec<AssertionTask>>,
        llm_judge_tasks: Option<Vec<LLMJudgeTask>>,
    ) -> Result<GenAIEvalProfile, DriftError> {
        let profile = GenAIEvalProfile::new(config, assertions_tasks, llm_judge_tasks)?;
        Ok(profile)
    }

    pub async fn compute_drift_single(
        record: GenAIEvalRecord,
        profile: &GenAIEvalProfile,
    ) -> Result<GenAIEvalSet, DriftError> {
        let profile = Arc::new(profile.clone());
        Ok(GenAIEvaluator::process_event_record(&record, profile).await?)
    }

    pub fn compute_drift(
        &mut self,
        data: Vec<GenAIEvalRecord>,
        profile: &GenAIEvalProfile,
    ) -> Result<Vec<GenAIEvalSet>, DriftError> {
        let results = app_state().handle().block_on(async move {
            let mut results = Vec::new();
            for record in data {
                let eval_set = Self::compute_drift_single(record, profile).await?;
                results.push(eval_set);
            }
            Ok::<_, DriftError>(results)
        })?; // propagate error from block_on

        Ok(results)
    }
}
