use crate::error::EvaluationError;
use crate::types::{EvaluationConfig, GenAIEvalTaskResult};
use crate::types::{GenAIEvalRecord, GenAIEvalResults};
use crate::util::{
    collect_evaluation_results, spawn_evaluation_tasks_with_embeddings,
    spawn_evaluation_tasks_without_embeddings,
};
use potato_head::{Agent, Provider, Task, Workflow, WorkflowError};
use pyo3::prelude::*;
use scouter_state::app_state;
use scouter_types::eval::GenAIEvalMetric;
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
pub async fn async_evaluate_genai(
    workflow: Workflow,
    records: Vec<GenAIEvalRecord>,
    config: &Arc<EvaluationConfig>,
) -> Result<GenAIEvalResults, EvaluationError> {
    debug!("Starting LLM evaluation for {} records", records.len());

    let join_set: JoinSet<(String, Option<GenAIEvalTaskResult>)> = match (
        config.embedder.as_ref(),
        config.embedding_targets.is_empty(),
    ) {
        (Some(embedder), false) => {
            debug!("Using embedding-enabled evaluation path");
            spawn_evaluation_tasks_with_embeddings(workflow, records, Arc::clone(embedder), config)
                .await
        }
        _ => {
            debug!("Using standard evaluation path");

            // this will return a list of VecEval
            spawn_evaluation_tasks_without_embeddings(workflow, records).await
        }
    };

    let results = collect_evaluation_results(join_set).await?;

    Ok(results)
}

/// Builds a workflow from a list of GenAIEvalMetric objects
pub async fn workflow_from_eval_metrics(
    eval_metrics: Vec<GenAIEvalMetric>,
    name: &str,
) -> Result<Workflow, EvaluationError> {
    // Build a workflow from metrics
    let mut workflow = Workflow::new(name);
    let mut agents: HashMap<Provider, Agent> = HashMap::new();
    let mut metric_names = Vec::new();

    // Create agents. We don't want to duplicate, so we check if the agent already exists.
    // if it doesn't, we create it.
    for metric in &eval_metrics {
        let agent = match agents.entry(metric.prompt.provider.clone()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let agent = Agent::new(metric.prompt.provider.clone(), None)
                    .await
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

#[pyfunction]
/// Function for evaluating LLM response and generating metrics.
/// The primary use case for evaluate_genai is to take a list of data samples, which often contain inputs and outputs
/// from LLM systems and evaluate them against user-defined metrics in a LLM as a judge pipeline. The user is expected provide
/// a list of dict objects and a list of LLMEval metrics. These eval metrics will be used to create a workflow, which is then
/// executed in an async context. All eval scores are extracted and returned to the user.
/// # Arguments
/// * `py`: The Python interpreter instance.
/// * `data`: A list of data samples to evaluate.
/// * `metrics`: A list of evaluation metrics to use.
#[pyo3(signature = (records, metrics, config=None))]
pub fn evaluate_genai(
    records: Vec<GenAIEvalRecord>,
    metrics: Vec<GenAIEvalMetric>,
    config: Option<EvaluationConfig>,
) -> Result<GenAIEvalResults, EvaluationError> {
    let config = Arc::new(config.unwrap_or_default());

    // Create runtime and execute evaluation pipeline
    let mut results = app_state().handle().block_on(async {
        let workflow = workflow_from_eval_metrics(metrics, "LLM Evaluation").await?;
        async_evaluate_genai(workflow, records, &config).await
    })?;

    // Only run post-processing if needed
    // Post processing includes calculating embedding means, similarities, clustering, and histograms
    if config.needs_post_processing() {
        results.finalize(&config)?;
    }

    Ok(results)
}
