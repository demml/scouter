use crate::error::EvaluationError;
use crate::types::{EvaluationConfig, LLMEvalRecord, LLMEvalResults, LLMEvalTaskResult};
use itertools::iproduct;
use linfa::{traits::*, Dataset};
use linfa_clustering::Dbscan;
use linfa_reduction::Pca;
use ndarray::{Array1, Array2};
use num_traits::FromPrimitive;
use potato_head::{
    Embedder, EmbeddingInput, PyEmbedder, Score, StructuredOutput, TaskStatus, Workflow,
    WorkflowError,
};
use pyo3::prelude::*;
use rayon::prelude::*;
use simsimd::SpatialSimilarity;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use tokio::task::JoinSet;
use tracing::{error, warn};
/// Process a workflow result and extract scores from completed tasks
pub fn process_workflow_result(
    workflow_result: Arc<RwLock<Workflow>>,
) -> Result<BTreeMap<String, Score>, EvaluationError> {
    let mut metrics = BTreeMap::new();

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
                        metrics.insert(task.id.clone(), score);
                    }
                    Err(e) => {
                        error!("Failed to validate score for task {}: {:?}", task.id, e);
                        // Continue processing other tasks instead of failing completely
                    }
                }
            }
        }
    }

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
                            Some(LLMEvalTaskResult::new(record_id, metrics, BTreeMap::new())),
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
    config: &Arc<EvaluationConfig>,
) -> JoinSet<(String, Option<LLMEvalTaskResult>)> {
    let mut join_set = JoinSet::new();

    for record in records {
        let inner_workflow = workflow.clone();
        let cloned_embedder = embedder.clone();
        let cloned_config = config.clone();

        join_set.spawn(async move {
            let record_id = record.id.clone();

            // Generate embeddings
            // We do this first because the workflow will consume the context
            let embeddings = generate_embeddings_for_record(
                &record,
                &cloned_embedder,
                &cloned_config.embedding_targets,
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
) -> BTreeMap<String, Vec<f32>> {
    let mut embeddings = BTreeMap::new();

    for target in embedding_targets {
        let texts = record
            .context
            .get(target)
            .and_then(|v| v.as_str())
            .map(|s| vec![s.to_string()]);

        if let Some(texts) = texts {
            match embedder.embed(EmbeddingInput::Texts(texts)).await {
                Ok(embedding_response) => match embedding_response.values() {
                    Ok(values) => {
                        // move ownership of values into Embedding struct
                        embeddings.insert(target.clone(), values.to_vec());
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
) -> Result<LLMEvalResults, EvaluationError> {
    let mut eval_results = LLMEvalResults::new();

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
            Some(py_embedder.embedder.clone())
        } else {
            // embedder provided but not a PyEmbedder instance
            return Err(EvaluationError::InvalidEmbedderType);
        }
    } else {
        None
    };
    Ok(embedder_arc)
}

/// Calculate the mean of for a slice of f32 values
/// There's no need for a generic implementation here, as we only need f32 for embeddings
pub fn compute_mean(vec: &[f32]) -> Option<f64> {
    match vec.len() {
        0 => None,
        _ => {
            let sum = vec.iter().sum::<f32>();
            let length = f32::from_usize(vec.len())?;

            let mean = sum / length;
            Some(mean as f64)
        }
    }
}

/// Uses the DBSCAN algorithm to cluster the provided data
/// Returns an array where each element corresponds to the cluster ID of the respective data point
pub fn cluster(data: &Array2<f64>) -> Result<Array1<Option<usize>>, EvaluationError> {
    let min_points = 3;
    let clusters = Dbscan::params(min_points).transform(data)?;
    Ok(clusters)
}

pub fn compute_similarity(
    targets: &Vec<String>,
    embeddings: &BTreeMap<String, Vec<f32>>,
    scores: &mut BTreeMap<String, f64>,
) {
    for (a, b) in iproduct!(targets, targets) {
        // only want unique pairs
        if a == b {
            continue;
        }
        if let (Some(vec_a), Some(vec_b)) = (embeddings.get(a), embeddings.get(b)) {
            if vec_a.len() != vec_b.len() {
                warn!(
                    "Embedding length mismatch for targets {} and {}: {} vs {}",
                    a,
                    b,
                    vec_a.len(),
                    vec_b.len()
                );
                continue;
            }

            let similarity = f32::cosine(vec_a, vec_b).unwrap_or(-1.0);
            let key = format!("{}_{}_cosine", a, b);
            scores.insert(key, similarity);
        } else {
            warn!("Missing embeddings for targets {} or {}", a, b);
        }
    }
}

pub fn post_process(results: &mut LLMEvalResults, config: &Arc<EvaluationConfig>) {
    // compute means for each embedding target
    results.results.par_iter_mut().for_each(|(_, task_result)| {
        for (target, values) in task_result.embedding.iter() {
            let mean = compute_mean(values).unwrap_or(-1.0);
            task_result.mean_embeddings.insert(target.clone(), mean);
        }
        compute_similarity(
            &config.embedding_targets,
            &task_result.embedding,
            &mut task_result.similarity_scores,
        );
    });
}

/// Reduced the dimensionality of the data using PCA.
/// This is needed to visualize the clusters in 2D space.
/// # Arguments
/// * `data` - The input data to reduce.
/// * `cluster_ids` - The cluster IDs for each data point.
pub fn reduce_dimensions(
    data: &Array2<f64>,
    cluster_ids: &[i32],
) -> Result<Array2<f64>, EvaluationError> {
    let ds = Dataset::new(
        data.clone(),
        Array2::from_shape_vec(
            (data.nrows(), 1),
            cluster_ids.iter().map(|&x| x as f64).collect(),
        )
        .unwrap(),
    );

    let embedding = Pca::params(2).whiten(false).fit(&ds)?;

    let prediction = embedding.predict(&ds);
    Ok(prediction)
}
