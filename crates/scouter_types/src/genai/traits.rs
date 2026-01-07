use crate::genai::{
    AssertionResult, AssertionTask, ComparisonOperator, EvaluationTask, EvaluationTaskType,
    LLMJudgeTask,
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

pub fn separate_tasks(tasks: Vec<EvaluationTask>) -> (Vec<AssertionTask>, Vec<LLMJudgeTask>) {
    let mut llm_judges = Vec::new();
    let mut assertions = Vec::new();

    for task in tasks {
        match task {
            EvaluationTask::Assertion(a) => assertions.push(a),
            EvaluationTask::LLMJudge(j) => llm_judges.push(*j),
        }
    }

    (assertions, llm_judges)
}

#[derive(Debug)]
pub enum TaskRef<'a> {
    Assertion(&'a mut AssertionTask),
    LLMJudge(&'a mut LLMJudgeTask),
}

impl<'a> TaskRef<'a> {
    pub fn depends_on(&self) -> &[String] {
        match self {
            TaskRef::Assertion(t) => t.depends_on(),
            TaskRef::LLMJudge(t) => t.depends_on(),
        }
    }
}

/// Extension trait for evaluation profiles
/// Provides unified access to assertions and LLM judge tasks
pub trait ProfileExt {
    fn id(&self) -> &str;
    fn get_assertion_by_id(&self, id: &str) -> Option<&AssertionTask>;
    fn get_llm_judge_by_id(&self, id: &str) -> Option<&LLMJudgeTask>;
    fn get_task_by_id(&self, id: &str) -> Option<&dyn TaskAccessor>;
    fn has_llm_tasks(&self) -> bool;
}
