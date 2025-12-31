use crate::tasks::evaluator::AssertionEvaluator;
use crate::{error::EvaluationError, tasks::traits::EvaluationTask};
use scouter_types::genai::{
    traits::TaskAccessor, AssertionResult, AssertionTask, EvaluationContext,
};
use serde_json::Value;

impl EvaluationTask for AssertionTask {
    fn execute(&self, context: &EvaluationContext) -> Result<AssertionResult, EvaluationError> {
        let task_context: &Value = if self.depends_on().is_empty() {
            &context.context
        } else {
            &context.build_merged_context(self.depends_on())?
        };
        let result = AssertionEvaluator::evaluate_assertion(task_context, self)?;

        Ok(result)
    }
}
