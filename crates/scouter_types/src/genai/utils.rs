use crate::error::TypeError;
use crate::genai::{AssertionTask, LLMJudgeTask, TraceAssertionTask};
use pyo3::prelude::*;
use pyo3::types::PyList;
use pythonize::depythonize;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct AssertionTasks {
    pub assertion: Vec<AssertionTask>,
    pub judge: Vec<LLMJudgeTask>,
    pub trace: Vec<TraceAssertionTask>,
}

impl AssertionTasks {
    pub fn collect_non_judge_task_ids(&self) -> BTreeSet<String> {
        self.assertion
            .iter()
            .map(|t| t.id.clone())
            .chain(self.trace.iter().map(|t| t.id.clone()))
            .collect()
    }

    pub fn collect_all_task_ids(&self) -> Result<BTreeSet<String>, TypeError> {
        let mut task_ids = BTreeSet::new();

        for task in &self.assertion {
            task_ids.insert(task.id.clone());
        }
        for task in &self.judge {
            task_ids.insert(task.id.clone());
        }
        for task in &self.trace {
            task_ids.insert(task.id.clone());
        }

        let total_tasks = self.assertion.len() + self.judge.len() + self.trace.len();
        if task_ids.len() != total_tasks {
            return Err(TypeError::DuplicateTaskIds);
        }

        Ok(task_ids)
    }
}

/// Helper function to extract AssertionTask and LLMJudgeTask from a PyList
pub fn extract_assertion_tasks_from_pylist(
    list: &Bound<'_, PyList>,
) -> Result<AssertionTasks, TypeError> {
    let mut assertion_tasks = Vec::new();
    let mut llm_judge_tasks = Vec::new();
    let mut trace_tasks = Vec::new();

    for item in list.iter() {
        if item.is_instance_of::<AssertionTask>() {
            let task = item.extract::<AssertionTask>()?;
            assertion_tasks.push(task);
        } else if item.is_instance_of::<LLMJudgeTask>() {
            let task = item.extract::<LLMJudgeTask>()?;
            llm_judge_tasks.push(task);
        } else if item.is_instance_of::<TraceAssertionTask>() {
            let task = item.extract::<TraceAssertionTask>()?;
            trace_tasks.push(task);
        } else {
            return Err(TypeError::InvalidAssertionTaskType);
        }
    }
    Ok(AssertionTasks {
        assertion: assertion_tasks,
        judge: llm_judge_tasks,
        trace: trace_tasks,
    })
}
