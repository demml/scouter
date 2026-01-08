use crate::error::EvaluationError;
use crate::tasks::evaluator::AssertionEvaluator;
use crate::tasks::traits::EvaluationTask;
use scouter_types::genai::{AssertionResult, LLMJudgeTask};
use serde_json::Value;

impl EvaluationTask for LLMJudgeTask {
    fn execute(&self, context: &Value) -> Result<AssertionResult, EvaluationError> {
        AssertionEvaluator::evaluate_assertion(context, self)
    }
}
