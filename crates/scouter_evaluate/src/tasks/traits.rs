use crate::error::EvaluationError;

use scouter_types::genai::{traits::TaskRef, AssertionResult};
use serde_json::Value;
use std::fmt::Debug;

pub trait EvaluationTask: Debug + Send + Sync {
    /// Execute the task and return results
    /// # Arguments
    /// * `context` - The evaluation context containing necessary data
    /// # Returns
    /// An EvaluationTaskResult containing the outcome of the task
    fn execute(&self, context: &Value) -> Result<AssertionResult, EvaluationError>;
}

/// Helper for mutably evaluation tasks for different task types
pub trait EvaluateTaskMut {
    fn evaluate_task(&self, context: &Value) -> Result<AssertionResult, EvaluationError>;
}

impl EvaluateTaskMut for TaskRef<'_> {
    fn evaluate_task(&self, context: &Value) -> Result<AssertionResult, EvaluationError> {
        let result = match self {
            TaskRef::Assertion(assertion) => assertion.execute(context)?,
            TaskRef::LLMJudge(judge) => judge.execute(context)?,
            TaskRef::TraceAssertion(trace) => trace.execute(context)?,
        };
        Ok(result)
    }
}
