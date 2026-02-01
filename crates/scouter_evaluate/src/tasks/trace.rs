use crate::evaluate::trace::TraceContextBuilder;
use crate::tasks::evaluator::AssertionEvaluator;
use crate::{error::EvaluationError, tasks::traits::EvaluationTask};
use pyo3::pyfunction;
use scouter_types::genai::{AssertionResult, AssertionResults, TraceAssertionTask};
use scouter_types::sql::TraceSpan;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

impl EvaluationTask for TraceAssertionTask {
    fn execute(&self, context: &Value) -> Result<AssertionResult, EvaluationError> {
        AssertionEvaluator::evaluate_assertion(context, self)
    }
}

#[pyfunction]
#[pyo3(signature = (tasks, spans))]
pub fn execute_trace_assertion_tasks(
    tasks: Vec<TraceAssertionTask>,
    spans: Vec<TraceSpan>,
) -> Result<AssertionResults, EvaluationError> {
    let context_builder = TraceContextBuilder::new(Arc::new(spans));

    let results: HashMap<String, AssertionResult> = tasks
        .iter()
        .map(|task| {
            let context = context_builder.build_context(&task.assertion)?;
            task.execute(&context)
                .map(|result| (task.id.clone(), result))
        })
        .collect::<Result<HashMap<String, AssertionResult>, EvaluationError>>()?;

    Ok(AssertionResults { results })
}
