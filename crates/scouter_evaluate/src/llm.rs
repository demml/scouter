use crate::error::EvaluationError;
use crate::types::LLMEvalTaskResult;
use crate::types::{LLMEvalRecord, LLMEvalResults};
use crate::util::{
    collect_evaluation_results, spawn_evaluation_tasks_with_embeddings,
    spawn_evaluation_tasks_without_embeddings,
};
use potato_head::agents::embed::PyEmbedder;
use potato_head::Embedder;
use potato_head::{Agent, Provider, Task, Workflow, WorkflowError};
use pyo3::prelude::*;
use scouter_types::eval::LLMEvalMetric;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::{debug, instrument};

/// Main orchestration function that decides which execution path to take
/// # Arguments
/// * `workflow`: The workflow to execute.
/// * `records`: The data records to evaluate.
/// * `embedder`: Optional embedder for embedding-based evaluations.
/// * `embedding_targets`: Optional list of fields to embed.
#[instrument(skip_all)]
async fn async_evaluate_llm(
    workflow: Workflow,
    records: Vec<LLMEvalRecord>,
    embedder: Option<Arc<Embedder>>,
    embedding_targets: Option<Vec<String>>,
) -> Result<LLMEvalResults, EvaluationError> {
    debug!("Starting LLM evaluation for {} records", records.len());

    let join_set: JoinSet<(String, Option<LLMEvalTaskResult>)> =
        match (embedder, &embedding_targets) {
            (Some(embedder), Some(targets)) => {
                debug!("Using embedding-enabled evaluation path");
                spawn_evaluation_tasks_with_embeddings(
                    workflow,
                    records,
                    embedder,
                    Arc::new(targets.clone()),
                )
                .await
            }
            _ => {
                debug!("Using standard evaluation path");

                // this will return a list of VecEval
                spawn_evaluation_tasks_without_embeddings(workflow, records).await
            }
        };

    let results = collect_evaluation_results(join_set, embedding_targets).await?;

    Ok(results)
}

/// Builds a workflow from a list of LLMEvalMetric objects
pub fn workflow_from_eval_metrics(
    eval_metrics: Vec<LLMEvalMetric>,
    name: &str,
) -> Result<Workflow, EvaluationError> {
    // Build a workflow from metrics
    let mut workflow = Workflow::new(name);
    let mut agents: HashMap<Provider, Agent> = HashMap::new();
    let mut metric_names = Vec::new();

    // Create agents. We don't want to duplicate, so we check if the agent already exists.
    // if it doesn't, we create it.
    for metric in &eval_metrics {
        let provider = metric.prompt.model_settings.provider();

        let agent = match agents.entry(provider) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let agent = Agent::from_model_settings(&metric.prompt.model_settings)
                    .map_err(|e| WorkflowError::Error(format!("Failed to create agent: {}", e)))?;
                workflow.add_agent(&agent);
                entry.insert(agent)
            }
        };

        let task = Task::new(&agent.id, metric.prompt.clone(), &metric.name, None, None);
        workflow.add_task(task)?;
        metric_names.push(metric.name.clone());
    }

    Ok(workflow)
}

/// Helper function for extracting embedder and runtime from optional PyEmbedder
fn parse_embedder(
    embedder: Option<&Bound<'_, PyAny>>,
) -> Result<(Option<Arc<Embedder>>, Arc<tokio::runtime::Runtime>), EvaluationError> {
    // Extract embedder and runtime if PyEmbedder is provided
    let (embedder_arc, runtime) = if let Some(embedder_bound) = embedder {
        if embedder_bound.is_instance_of::<PyEmbedder>() {
            let py_embedder = embedder_bound.extract::<PyEmbedder>()?;
            let embedder_arc = Some(py_embedder.embedder.clone());
            let runtime = py_embedder.runtime.clone();
            (embedder_arc, runtime)
        } else {
            // embedder provided but not a PyEmbedder instance
            return Err(EvaluationError::InvalidEmbedderType);
        }
    } else {
        // No embedder provided, create new runtime
        let runtime = Arc::new(tokio::runtime::Runtime::new()?);
        (None, runtime)
    };
    Ok((embedder_arc, runtime))
}

#[pyfunction]
/// Function for evaluating LLM response and generating metrics.
/// The primary use case for evaluate_llm is to take a list of data samples, which often contain inputs and outputs
/// from LLM systems and evaluate them against user-defined metrics in a LLM as a judge pipeline. The user is expected provide
/// a list of dict objects and a list of LLMEval metrics. These eval metrics will be used to create a workflow, which is then
/// executed in an async context. All eval scores are extracted and returned to the user.
/// # Arguments
/// * `py`: The Python interpreter instance.
/// * `data`: A list of data samples to evaluate.
/// * `metrics`: A list of evaluation metrics to use.
#[pyo3(signature = (records, metrics, embedder=None, embedding_targets=None))]
pub fn evaluate_llm(
    records: Vec<LLMEvalRecord>,
    metrics: Vec<LLMEvalMetric>,
    embedder: Option<&Bound<'_, PyAny>>,
    embedding_targets: Option<Vec<String>>,
) -> Result<LLMEvalResults, EvaluationError> {
    let workflow = workflow_from_eval_metrics(metrics, "LLM Evaluation")?;

    // Extract embedder and runtime if PyEmbedder is provided
    let (embedder_arc, runtime) = parse_embedder(embedder)?;

    let results = runtime.block_on(async {
        async_evaluate_llm(workflow, records, embedder_arc, embedding_targets).await
    })?;

    Ok(results)
}
