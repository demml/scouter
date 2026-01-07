use crate::error::TypeError;
use crate::genai::{AssertionTask, ConditionalTask, LLMJudgeTask};
use pyo3::prelude::*;
use pyo3::types::PyList;

/// Helper function to extract AssertionTask and LLMJudgeTask from a PyList
pub fn extract_assertion_tasks_from_pylist(
    list: &Bound<'_, PyList>,
) -> Result<(Vec<AssertionTask>, Vec<LLMJudgeTask>, Vec<ConditionalTask>), TypeError> {
    let mut assertion_tasks = Vec::new();
    let mut llm_judge_tasks = Vec::new();
    let mut conditional_tasks = Vec::new();

    for item in list.iter() {
        if item.is_instance_of::<AssertionTask>() {
            let task = item.extract::<AssertionTask>()?;
            assertion_tasks.push(task);
        } else if item.is_instance_of::<LLMJudgeTask>() {
            let task = item.extract::<LLMJudgeTask>()?;
            llm_judge_tasks.push(task);
        } else if item.is_instance_of::<ConditionalTask>() {
            let task = item.extract::<ConditionalTask>()?;
            conditional_tasks.push(task);
        } else {
            return Err(TypeError::InvalidAssertionTaskType);
        }
    }
    Ok((assertion_tasks, llm_judge_tasks, conditional_tasks))
}
