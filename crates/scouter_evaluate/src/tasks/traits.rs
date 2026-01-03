use crate::error::EvaluationError;
use scouter_types::genai::traits::TaskAccessor;
use scouter_types::genai::{traits::TaskRefMut, AssertionResult, EvaluationContext};
use std::fmt::Debug;

pub trait EvaluationTask: Debug + Send + Sync {
    /// Execute the task and return results
    /// # Arguments
    /// * `context` - The evaluation context containing necessary data
    /// # Returns
    /// An EvaluationTaskResult containing the outcome of the task
    fn execute(&self, context: &EvaluationContext) -> Result<AssertionResult, EvaluationError>;
}

/// Helper for mutably evaluation tasks for different task types
pub trait EvaluateTaskMut {
    fn evaluate_task_mut(&mut self, context: &EvaluationContext) -> Result<(), EvaluationError>;
}

impl EvaluateTaskMut for TaskRefMut<'_> {
    fn evaluate_task_mut(&mut self, context: &EvaluationContext) -> Result<(), EvaluationError> {
        match self {
            TaskRefMut::Assertion(assertion) => {
                let result = assertion.execute(context)?;
                assertion.add_result(result);
            }
            TaskRefMut::LLMJudge(judge) => {
                let result = judge.execute(context)?;
                judge.add_result(result);
            }
        };
        Ok(())
    }
}
