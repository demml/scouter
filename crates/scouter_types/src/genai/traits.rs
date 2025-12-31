use crate::genai::{AssertionTask, ComparisonOperator, LLMJudgeTask};
use serde_json::Value;
use std::fmt::Debug;
/// Base trait for all evaluation tasks

pub trait TaskAccessor {
    /// Returns optional field path - avoids `&Option<String>` pattern
    fn field_path(&self) -> Option<&str>;

    /// Returns assertion ID as string slice
    fn id(&self) -> &str;

    /// Returns reference to comparison operator
    fn operator(&self) -> &ComparisonOperator;

    /// Returns reference to expected value
    fn expected_value(&self) -> &Value;

    /// Returns slice of dependency IDs - more efficient than `&Vec<String>`
    fn depends_on(&self) -> &[String];
}

#[derive(Debug, Clone, Copy)]
pub enum TaskRef<'a> {
    Assertion(&'a AssertionTask),
    LLMJudge(&'a LLMJudgeTask),
}

impl<'a> TaskRef<'a> {
    /// Access the underlying task through the TaskAccessor trait
    #[inline]
    pub fn as_task(&self) -> &dyn TaskAccessor {
        match self {
            TaskRef::Assertion(t) => *t,
            TaskRef::LLMJudge(t) => *t,
        }
    }
}

/// Extension trait for evaluation profiles
/// Provides unified access to assertions and LLM judge tasks
pub trait ProfileExt {
    fn id(&self) -> &str;
    fn get_task_by_id(&self, id: &str) -> Option<TaskRef>;
    fn get_assertion_by_id(&self, id: &str) -> Option<&AssertionTask>;
    fn get_llm_judge_by_id(&self, id: &str) -> Option<&LLMJudgeTask>;

    fn has_llm_tasks(&self) -> bool;
}
