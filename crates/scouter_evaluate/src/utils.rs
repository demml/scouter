use crate::agent::EvalDataset;
use crate::error::EvaluationError;
use crate::evaluate::evaluator::AgentEvaluator;
use crate::evaluate::types::{EvalResults, EvaluationConfig};
use crate::tasks::evaluator::FieldEvaluator;
use itertools::iproduct;
use num_traits::FromPrimitive;
use potato_head::{Embedder, EmbeddingInput, PyEmbedder};
use pyo3::prelude::*;
use rayon::prelude::*;
use scouter_types::agent::EvalSet;
use scouter_types::EvalRecord;
use serde_json::Value;
use simsimd::SpatialSimilarity;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::task::JoinSet;
use tracing::{debug, error, warn};

type EvalTaskResult = (
    usize, // Index into records array
    Result<(EvalSet, BTreeMap<String, Vec<f32>>), String>,
);

/// Spawn tasks without embedding support
/// This function will spawn a task that runs the workflows and extracts the results
/// If there is an error during workflow execution, it will log the error and return None for that record
/// # Arguments
/// * `workflow` - The workflow to execute for each record.
/// * `records` - The list of EvalRecords to process.
/// # Returns
/// A JoinSet containing tuples of record ID and optional EvalTaskResult.
pub async fn spawn_evaluation_tasks_without_embeddings(
    dataset: &EvalDataset,
    _config: &Arc<EvaluationConfig>,
) -> JoinSet<EvalTaskResult> {
    let mut join_set = JoinSet::new();

    for (idx, _) in dataset.records.iter().enumerate() {
        // cloning here so we can reference inside async move
        let record_ref = dataset.records.clone();
        let profile_ref = dataset.profile.clone();
        let spans_ref = dataset.spans.clone();

        join_set.spawn(async move {
            // Access record by index - no cloning
            let record = &record_ref[idx];

            debug!(
                "Starting evaluation for record {} and index {}",
                record.uid, idx
            );

            let result =
                match AgentEvaluator::process_event_record(record, profile_ref, spans_ref).await {
                    Ok(eval_set) => Ok((eval_set, BTreeMap::new())),
                    Err(e) => Err(format!("Evaluation failed: {}", e)),
                };

            (idx, result)
        });
    }

    join_set
}

/// Spawn tasks to run evaluation workflows with embedding calculations
/// # Arguments
/// * `dataset` - The EvalDataset containing records to evaluate.
/// * `embedder` - The Embedder instance to use for generating embeddings.
/// * `config` - The EvaluationConfig containing evaluation settings.
/// # Returns
/// A JoinSet containing GenAIEvalTaskResults for each record.
pub async fn spawn_evaluation_tasks_with_embeddings(
    dataset: &EvalDataset,
    embedder: Arc<Embedder>,
    config: &Arc<EvaluationConfig>,
) -> JoinSet<EvalTaskResult> {
    let mut join_set = JoinSet::new();

    for (idx, _) in dataset.records.iter().enumerate() {
        let record_ref = dataset.records.clone();
        let profile_ref = dataset.profile.clone();
        let spans_ref = dataset.spans.clone();
        let embedder_ref = embedder.clone();
        let config_ref = config.clone();

        join_set.spawn(async move {
            let record = &record_ref[idx];

            // Generate embeddings
            let embeddings = generate_embeddings_for_record(
                record,
                &embedder_ref,
                &config_ref.embedding_targets,
            )
            .await;

            // Execute evaluation
            let result =
                match AgentEvaluator::process_event_record(record, profile_ref, spans_ref).await {
                    Ok(eval_set) => Ok((eval_set, embeddings)),
                    Err(e) => Err(format!("Evaluation failed: {}", e)),
                };

            (idx, result)
        });
    }

    join_set
}

/// Helper for extracting embeddings for a single record. Used in the genai evaulation workflow.
/// # Arguments
/// * `record` - The EvalRecord to extract embeddings from.
/// * `embedder` - The Embedder instance to use for generating embeddings.
/// * `embedding_targets` - The list of keys in the record's context to generate embeddings for.
/// # Returns
pub async fn generate_embeddings_for_record(
    record: &EvalRecord,
    embedder: &Arc<Embedder>,
    embedding_targets: &[String],
) -> BTreeMap<String, Vec<f32>> {
    let mut embeddings = BTreeMap::new();

    for target in embedding_targets {
        match FieldEvaluator::extract_field_value(&record.context, target) {
            Ok(value) => {
                let text = match value {
                    Value::String(s) => Some(s.clone()),
                    Value::Array(_) | Value::Object(_) => serde_json::to_string(value).ok(),
                    _ => {
                        warn!(
                            "Field '{}' has unsupported type for embedding: {:?}",
                            target, value
                        );
                        None
                    }
                };

                if let Some(text) = text {
                    match embedder.embed(EmbeddingInput::Texts(vec![text])).await {
                        Ok(embedding_response) => match embedding_response.values() {
                            Ok(values) => {
                                embeddings.insert(target.clone(), values.to_vec());
                            }
                            Err(e) => {
                                error!(
                                    "Failed to extract embedding values for target '{}': {:?}",
                                    target, e
                                );
                            }
                        },
                        Err(e) => {
                            error!(
                                "Failed to generate embedding for target '{}': {:?}",
                                target, e
                            );
                        }
                    }
                }
            }
            Err(e) => {
                warn!("Failed to extract field '{}' for embedding: {}", target, e);
            }
        }
    }

    embeddings
}
/// Collect and align results with original records
pub async fn collect_and_align_results(
    mut join_set: JoinSet<EvalTaskResult>,
    records: &Arc<Vec<EvalRecord>>,
) -> Result<EvalResults, EvaluationError> {
    let mut results = EvalResults::new();

    while let Some(join_result) = join_set.join_next().await {
        match join_result {
            Ok((idx, eval_result)) => {
                let record = &records[idx];

                match eval_result {
                    Ok((eval_set, embeddings)) => {
                        results.add_success(record, eval_set, embeddings);
                    }
                    Err(error_msg) => {
                        results.add_failure(record, error_msg);
                    }
                }
            }
            Err(join_error) => {
                error!("Task join error: {:?}", join_error);
            }
        }
    }

    Ok(results)
}

/// Post-process aligned results
pub fn post_process_aligned_results(
    results: &mut EvalResults,
    config: &Arc<EvaluationConfig>,
) -> Result<(), EvaluationError> {
    results.aligned_results.par_iter_mut().for_each(|aligned| {
        // Compute embedding means
        for (target, values) in aligned.embeddings.iter() {
            if let Some(mean) = compute_mean(values) {
                aligned.mean_embeddings.insert(target.clone(), mean);
            }
        }

        // Compute similarities
        if config.compute_similarity {
            compute_similarity(
                &config.embedding_targets,
                &aligned.embeddings,
                &mut aligned.similarity_scores,
            );
        }
    });

    Ok(())
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
