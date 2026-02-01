use crate::genai::{
    utils::AssertionTasks, AssertionResult, AssertionTask, ComparisonOperator, EvaluationTask,
    EvaluationTaskType, LLMJudgeTask, TraceAssertionTask,
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

pub fn separate_tasks(tasks: Vec<EvaluationTask>) -> AssertionTasks {
    let mut llm_judges = Vec::new();
    let mut assertions = Vec::new();
    let mut trace_assertions = Vec::new();

    for task in tasks {
        match task {
            EvaluationTask::Assertion(a) => assertions.push(*a),
            EvaluationTask::LLMJudge(j) => llm_judges.push(*j),
            EvaluationTask::TraceAssertion(t) => trace_assertions.push(*t),
            _ => todo!("Handle other task types"),
        }
    }

    AssertionTasks {
        assertion: assertions,
        judge: llm_judges,
        trace: trace_assertions,
    }
}

#[derive(Debug)]
pub enum TaskRef<'a> {
    Assertion(&'a mut AssertionTask),
    LLMJudge(&'a mut LLMJudgeTask),
    TraceAssertion(&'a mut TraceAssertionTask),
}

impl<'a> TaskRef<'a> {
    pub fn depends_on(&self) -> &[String] {
        match self {
            TaskRef::Assertion(t) => t.depends_on(),
            TaskRef::LLMJudge(t) => t.depends_on(),
            TaskRef::TraceAssertion(t) => t.depends_on(),
        }
    }
}

/// Extension trait for evaluation profiles
/// Provides unified access to assertions and LLM judge tasks
pub trait ProfileExt {
    fn id(&self) -> &str;
    fn get_assertion_by_id(&self, id: &str) -> Option<&AssertionTask>;
    fn get_llm_judge_by_id(&self, id: &str) -> Option<&LLMJudgeTask>;
    fn get_trace_assertion_by_id(&self, id: &str) -> Option<&TraceAssertionTask>;
    fn get_task_by_id(&self, id: &str) -> Option<&dyn TaskAccessor>;
    fn has_llm_tasks(&self) -> bool;
    fn has_trace_assertions(&self) -> bool;
}
