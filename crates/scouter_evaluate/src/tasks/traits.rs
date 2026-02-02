use crate::error::EvaluationError;

use scouter_types::genai::AssertionResult;
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
