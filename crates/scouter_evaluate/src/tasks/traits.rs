use crate::error::EvaluationError;
use scouter_types::genai::{EvaluationContext, EvaluationTaskResult};
use std::fmt::Debug;
/// Base trait for all evaluation tasks
pub trait EvaluationTask: Debug + Send + Sync {
    /// Execute the task and return results
    /// # Arguments
    /// * `context` - The evaluation context containing necessary data
    /// # Returns
    /// An EvaluationTaskResult containing the outcome of the task
    fn execute(&self, context: &EvaluationContext)
        -> Result<EvaluationTaskResult, EvaluationError>;
}
