use crate::error::EvaluationError;
use crate::types::{Embedding, LLMEvalRecord, LLMEvalResults, LLMEvalTaskResult, MetricResult};
use linfa::traits::*;
use linfa_clustering::{Dbscan, DbscanParams};
use potato_head::{
    Embedder, PyEmbedder, Score, StructuredOutput, TaskStatus, Workflow, WorkflowError,
};
use pyo3::prelude::*;
use std::sync::{Arc, RwLock};
use tokio::task::JoinSet;
use tracing::{error, warn};
/// Process a workflow result and extract scores from completed tasks
pub fn process_workflow_result(
    workflow_result: Arc<RwLock<Workflow>>,
) -> Result<Vec<MetricResult>, EvaluationError> {
    let mut metrics = Vec::new();

    let workflow = workflow_result
        .read()
        .map_err(|_| WorkflowError::LockAcquireError)?;

    let tasks = workflow.task_list.tasks();

    // iterate of each task and extract score
    for task in tasks.values() {
        if let (TaskStatus::Completed, Some(result)) = (&task.status, &task.result) {
            if let Some(content) = result.content() {
                match Score::model_validate_json_str(&content) {
                    Ok(score) => {
                        metrics.push(MetricResult {
                            task: task.id.clone(),
                            score,
                        });
                    }
                    Err(e) => {
                        error!("Failed to validate score for task {}: {:?}", task.id, e);
                        // Continue processing other tasks instead of failing completely
                    }
                }
            }
        }
    }

    // ensure consistent ordering by task name
    metrics.sort_by(|a, b| a.task.cmp(&b.task));
    Ok(metrics)
}

/// Spawn tasks without embedding support
/// This function will spawn a task that runs the workflows and extracts the results
/// If there is an error during workflow execution, it will log the error and return None for that record
/// # Arguments
/// * `workflow` - The workflow to execute for each record.
/// * `records` - The list of LLMEvalRecords to process.
/// # Returns
/// A JoinSet containing tuples of record ID and optional LLMEvalTaskResult.
pub async fn spawn_evaluation_tasks_without_embeddings(
    workflow: Workflow,
    records: Vec<LLMEvalRecord>,
) -> JoinSet<(String, Option<LLMEvalTaskResult>)> {
    let mut join_set = JoinSet::new();

    for record in records {
        let inner_workflow = workflow.clone();

        join_set.spawn(async move {
            let record_id = record.id.clone();

            match inner_workflow.run(Some(record.context)).await {
                Ok(workflow_result) => {
                    // parse the workflow result
                    match process_workflow_result(workflow_result) {
                        Ok(metrics) => (
                            record_id.clone(),
                            Some(LLMEvalTaskResult::new(record_id, metrics, vec![])),
                        ),
                        Err(error) => {
                            error!(
                                "Failed to process workflow result for record {}: {}",
                                record_id, error
                            );
                            (record_id, None)
                        }
                    }
                }
                Err(workflow_error) => {
                    error!(
                        "Workflow execution failed for record {}: {}",
                        record_id, workflow_error
                    );
                    (record_id, None)
                }
            }
        });
    }

    join_set
}

/// Spawn tasks to run evaluation workflows with embedding calculations
/// # Arguments
/// * `workflow` - The workflow to execute for each record.
/// * `records` - The list of LLMEvalRecords to process.
/// * `embedder` - The Embedder instance to use for generating embeddings.
/// * `embedding_targets` - The list of keys in the record's context to generate embeddings.
/// # Returns
/// A JoinSet containing LLMEvalTaskResults for each record.
pub async fn spawn_evaluation_tasks_with_embeddings(
    workflow: Workflow,
    records: Vec<LLMEvalRecord>,
    embedder: Arc<Embedder>,
    embedding_targets: Arc<Vec<String>>,
) -> JoinSet<(String, Option<LLMEvalTaskResult>)> {
    let mut join_set = JoinSet::new();

    for record in records {
        let inner_workflow = workflow.clone();
        let cloned_embedder = embedder.clone();
        let cloned_embedding_targets = embedding_targets.clone();

        join_set.spawn(async move {
            let record_id = record.id.clone();

            // Generate embeddings
            // We do this first because the workflow will consume the context
            let embeddings = generate_embeddings_for_record(
                &record,
                &cloned_embedder,
                &cloned_embedding_targets,
            )
            .await;

            // Run workflow
            match inner_workflow.run(Some(record.context)).await {
                Ok(workflow_result) => {
                    // parse the workflow result
                    match process_workflow_result(workflow_result) {
                        Ok(metrics) => (
                            record_id.clone(),
                            Some(LLMEvalTaskResult::new(record_id, metrics, embeddings)),
                        ),
                        Err(error) => {
                            error!(
                                "Failed to process workflow result for record {}: {}",
                                record_id, error
                            );
                            (record_id, None)
                        }
                    }
                }
                Err(workflow_error) => {
                    error!(
                        "Workflow execution failed for record {}: {}",
                        record_id, workflow_error
                    );
                    (record_id, None)
                }
            }
        });
    }

    join_set
}

/// Helper for extracting embeddings for a single record. Used in the llm evaulation workflow.
/// # Arguments
/// * `record` - The LLMEvalRecord to extract embeddings from.
/// * `embedder` - The Embedder instance to use for generating embeddings.
/// * `embedding_targets` - The list of keys in the record's context to generate embeddings for.
/// # Returns
pub async fn generate_embeddings_for_record(
    record: &LLMEvalRecord,
    embedder: &Arc<Embedder>,
    embedding_targets: &[String],
) -> Vec<Embedding> {
    let mut embeddings = Vec::new();

    for target in embedding_targets {
        let texts = record
            .context
            .get(target)
            .and_then(|v| v.as_str())
            .map(|s| vec![s.to_string()]);

        if let Some(texts) = texts {
            match embedder.embed(texts).await {
                Ok(embedding_response) => match embedding_response.values() {
                    Ok(values) => {
                        embeddings.push(Embedding::new(target.clone(), values.to_vec()));
                    }
                    Err(e) => {
                        error!(
                            "Failed to extract embedding values for target {}: {:?}",
                            target, e
                        );
                    }
                },
                Err(e) => {
                    error!(
                        "Failed to generate embedding for target {}: {:?}",
                        target, e
                    );
                }
            }
        } else {
            warn!("No text found for embedding target: {}", target);
        }
    }

    embeddings
}

/// Enhanced result collection with proper error handling
pub async fn collect_evaluation_results(
    mut join_set: JoinSet<(String, Option<LLMEvalTaskResult>)>,
    embedding_targets: Arc<Vec<String>>,
) -> Result<LLMEvalResults, EvaluationError> {
    let mut eval_results = LLMEvalResults::new(embedding_targets);

    while let Some(join_result) = join_set.join_next().await {
        match join_result {
            Ok(task_result) => {
                let (record_id, task_result_opt) = task_result;
                if let Some(task_result) = task_result_opt {
                    eval_results.results.entry(record_id).or_insert(task_result);
                } else {
                    eval_results.errored_tasks.push(record_id);
                }
            }
            Err(join_error) => {
                error!("Task join error: {:?}", join_error);
                // We can't associate this error with a specific record ID
            }
        }
    }

    Ok(eval_results)
}

/// Helper function for extracting embedder and runtime from optional PyEmbedder
/// # Arguments
/// * `embedder` - Optional reference to a PyEmbedder instance.
/// # Returns
/// An optional Arc-wrapped Embedder instance if provided, otherwise None.
pub fn parse_embedder(
    embedder: Option<&Bound<'_, PyAny>>,
) -> Result<Option<Arc<Embedder>>, EvaluationError> {
    // Extract embedder and runtime if PyEmbedder is provided
    let embedder_arc = if let Some(embedder_bound) = embedder {
        if embedder_bound.is_instance_of::<PyEmbedder>() {
            let py_embedder = embedder_bound.extract::<PyEmbedder>()?;
            let embedder_arc = Some(py_embedder.embedder.clone());
            embedder_arc
        } else {
            // embedder provided but not a PyEmbedder instance
            return Err(EvaluationError::InvalidEmbedderType);
        }
    } else {
        None
    };
    Ok(embedder_arc)
}

//pub fn cluster() {
//    let min_points = 3;
//    let clusters = Dbscan::params(min_points)
//        .tolerance(1e-2)
//        .transform(&observations)
//        .unwrap();
//}
