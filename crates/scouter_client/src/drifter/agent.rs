use scouter_drift::error::DriftError;
use scouter_evaluate::evaluate::AgentEvaluator;
use scouter_state::app_state;
use scouter_types::agent::{AgentEvalProfile, EvalSet};
use scouter_types::EvalRecord;
use std::sync::Arc;
/// Using "ClientAgentDrifter" to avoid confusion with the server-side GenAIDrifter
pub struct ClientAgentDrifter {}

impl Default for ClientAgentDrifter {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientAgentDrifter {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn compute_drift_single(
        record: EvalRecord,
        profile: &AgentEvalProfile,
    ) -> Result<EvalSet, DriftError> {
        let profile = Arc::new(profile.clone());
        Ok(AgentEvaluator::process_event_record(&record, profile, Arc::new(vec![])).await?)
    }

    pub fn compute_drift(
        &mut self,
        data: Vec<EvalRecord>,
        profile: &AgentEvalProfile,
    ) -> Result<Vec<EvalSet>, DriftError> {
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
