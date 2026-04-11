use crate::evaluate::agent::AgentContextBuilder;
use crate::tasks::evaluator::AssertionEvaluator;
use crate::{error::EvaluationError, tasks::traits::EvaluationTask};
use pyo3::prelude::*;
use pyo3::pyfunction;
use pythonize::depythonize;
use scouter_types::agent::{AgentAssertionTask, AssertionResult, AssertionResults};
use serde_json::Value;
use std::collections::HashMap;

impl EvaluationTask for AgentAssertionTask {
    fn execute(&self, context: &Value) -> Result<AssertionResult, EvaluationError> {
        AssertionEvaluator::evaluate_assertion(context, self)
    }
}

#[pyfunction]
#[pyo3(signature = (tasks, context))]
pub fn execute_agent_assertion_tasks(
    tasks: Vec<AgentAssertionTask>,
    context: &Bound<'_, PyAny>,
) -> Result<AssertionResults, EvaluationError> {
    let context: serde_json::Value = depythonize(context).map_err(|e| {
        EvaluationError::AgentEvaluatorError(format!("Failed to deserialize context: {}", e))
    })?;
    let results: HashMap<String, AssertionResult> = tasks
        .iter()
        .map(|task| {
            let context_builder = AgentContextBuilder::from_context(
                &context,
                task.provider.as_ref(),
                task.context_path.as_deref(),
            )?;
            let resolved = context_builder.build_context(&task.assertion)?;
            task.execute(&resolved)
                .map(|result| (task.id.clone(), result))
        })
        .collect::<Result<HashMap<String, AssertionResult>, EvaluationError>>()?;

    Ok(AssertionResults { results })
}
