use crate::tasks::evaluator::AssertionEvaluator;
use crate::{error::EvaluationError, tasks::traits::EvaluationTask};
use scouter_types::genai::{AssertionResult, TraceAssertionTask};
use serde_json::Value;

impl EvaluationTask for TraceAssertionTask {
    fn execute(&self, context: &Value) -> Result<AssertionResult, EvaluationError> {
        AssertionEvaluator::evaluate_assertion(context, self)
    }
}
