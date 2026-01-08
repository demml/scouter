use crate::tasks::evaluator::AssertionEvaluator;
use crate::{error::EvaluationError, tasks::traits::EvaluationTask};
use scouter_types::genai::AssertionResult;
use scouter_types::genai::AssertionTask;
use serde_json::Value;

impl EvaluationTask for AssertionTask {
    fn execute(&self, context: &Value) -> Result<AssertionResult, EvaluationError> {
        AssertionEvaluator::evaluate_assertion(context, self)
    }
}
