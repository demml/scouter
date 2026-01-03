use crate::genai::{
    AssertionResult, AssertionTask, ComparisonOperator, EvaluationTaskType, LLMJudgeTask,
};
use serde_json::Value;
use std::fmt::Debug;

pub trait TaskAccessor {
    /// Returns optional field path - avoids `&Option<String>` pattern
    fn field_path(&self) -> Option<&str>;

    /// Returns assertion ID as string slice
    fn id(&self) -> &str;

    fn task_type(&self) -> &EvaluationTaskType;

    /// Returns reference to comparison operator
    fn operator(&self) -> &ComparisonOperator;

    /// Returns reference to expected value
    fn expected_value(&self) -> &Value;

    /// Returns slice of dependency IDs - more efficient than `&Vec<String>`
    fn depends_on(&self) -> &[String];

    fn add_result(&mut self, result: AssertionResult);
}

#[derive(Debug)]
pub enum TaskRefMut<'a> {
    Assertion(&'a mut AssertionTask),
    LLMJudge(&'a mut LLMJudgeTask),
}

impl<'a> TaskRefMut<'a> {
    pub fn add_result(&mut self, result: AssertionResult) {
        match self {
            TaskRefMut::Assertion(t) => t.add_result(result),
            TaskRefMut::LLMJudge(t) => t.add_result(result),
        }
    }
}

/// Extension trait for evaluation profiles
/// Provides unified access to assertions and LLM judge tasks
pub trait ProfileExt {
    fn id(&self) -> &str;
    fn get_task_by_id_mut(&'_ mut self, id: &str) -> Option<TaskRefMut<'_>>;
    fn get_assertion_by_id(&self, id: &str) -> Option<&AssertionTask>;
    fn get_llm_judge_by_id(&self, id: &str) -> Option<&LLMJudgeTask>;

    fn has_llm_tasks(&self) -> bool;
}
