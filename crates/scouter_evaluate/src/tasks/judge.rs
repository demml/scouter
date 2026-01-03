use crate::error::EvaluationError;
use crate::tasks::evaluator::AssertionEvaluator;
use crate::tasks::traits::EvaluationTask;
use scouter_types::genai::{
    traits::TaskAccessor, AssertionResult, EvaluationContext, LLMJudgeTask,
};
use serde_json::Value;

impl EvaluationTask for LLMJudgeTask {
    fn execute(&self, context: &EvaluationContext) -> Result<AssertionResult, EvaluationError> {
        // we dont want to clone here if we can avoid it
        let task_context: &Value = if self.depends_on().is_empty() {
            &context.context
        } else {
            &context.build_merged_context(self.depends_on())?
        };
        AssertionEvaluator::evaluate_assertion(task_context, self)
    }
}
