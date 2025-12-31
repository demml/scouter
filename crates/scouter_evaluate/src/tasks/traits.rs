use crate::error::EvaluationError;
use scouter_types::genai::{AssertionResult, EvaluationContext};
use serde_json::Value;
use std::fmt::Debug;
/// Base trait for all evaluation tasks

pub trait AssertionAccessor {
    fn field_path(&self) -> &Option<String>;

    fn id(&self) -> &String;

    fn operator(&self) -> &scouter_types::genai::ComparisonOperator;

    fn expected_value(&self) -> &Value;

    fn depends_on(&self) -> &Vec<String>;
}

pub trait EvaluationTask: Debug + Send + Sync {
    /// Execute the task and return results
    /// # Arguments
    /// * `context` - The evaluation context containing necessary data
    /// # Returns
    /// An EvaluationTaskResult containing the outcome of the task
    fn execute(&self, context: &EvaluationContext) -> Result<AssertionResult, EvaluationError>;
}
