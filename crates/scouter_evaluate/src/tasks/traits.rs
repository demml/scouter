use crate::error::EvaluationError;

use scouter_types::genai::{traits::TaskRef, AssertionResult, EvaluationContext};
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
    fn evaluate_task(
        &self,
        context: &EvaluationContext,
    ) -> Result<AssertionResult, EvaluationError>;
}

impl EvaluateTaskMut for TaskRef<'_> {
    fn evaluate_task(
        &self,
        context: &EvaluationContext,
    ) -> Result<AssertionResult, EvaluationError> {
        let result = match self {
            TaskRef::Assertion(assertion) => assertion.execute(context)?,
            TaskRef::LLMJudge(judge) => judge.execute(context)?,
        };
        Ok(result)
    }
}
