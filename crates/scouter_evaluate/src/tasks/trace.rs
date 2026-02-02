use crate::evaluate::trace::TraceContextBuilder;
use crate::tasks::evaluator::AssertionEvaluator;
use crate::{error::EvaluationError, tasks::traits::EvaluationTask};
use pyo3::pyfunction;
use scouter_types::genai::{AssertionResult, AssertionResults, TraceAssertionTask};
use scouter_types::sql::TraceSpan;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::error;

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

pub(crate) fn execute_trace_assertions(
    context: &TraceContextBuilder,
    tasks: &[TraceAssertionTask],
) -> Result<AssertionResults, EvaluationError> {
    if tasks.is_empty() {
        return Ok(AssertionResults {
            results: HashMap::new(),
        });
    }

    // If there are tasks but no spans, fail all assertions
    if context.spans.is_empty() {
        let results: HashMap<String, AssertionResult> = tasks
            .iter()
            .map(|task| {
                (
                    task.id.clone(),
                    AssertionResult {
                        passed: false,
                        actual: serde_json::Value::Null,
                        expected: serde_json::Value::Null,
                        message: "No spans available for evaluation".to_string(),
                    },
                )
            })
            .collect();

        return Ok(AssertionResults { results });
    }

    let results: HashMap<String, AssertionResult> = tasks
        .iter()
        .map(|task| {
            let context = context.build_context(&task.assertion)?;
            task.execute(&context)
                .map(|result| (task.id.clone(), result))
        })
        .collect::<Result<HashMap<String, AssertionResult>, EvaluationError>>()
        .inspect_err(|e| {
            error!("Error executing trace assertions: {:?}", e);
        })?;

    Ok(AssertionResults { results })
}
