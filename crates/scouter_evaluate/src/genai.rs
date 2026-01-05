use crate::error::EvaluationError;
use crate::types::{EvaluationConfig, GenAIEvalResults};
use crate::utils::{
    collect_and_align_results, post_process_aligned_results,
    spawn_evaluation_tasks_with_embeddings, spawn_evaluation_tasks_without_embeddings,
};
use scouter_types::genai::GenAIEvalDataset;
use std::sync::Arc;
use tracing::{debug, instrument};

/// Main orchestration function that decides which execution path to take
/// # Arguments
/// * `dataset`: The dataset containing records to evaluate.
/// * `embedding_targets`: Optional list of fields to embed.
#[instrument(skip_all)]
pub async fn evaluate_genai_dataset(
    dataset: GenAIEvalDataset,
    config: &Arc<EvaluationConfig>,
) -> Result<GenAIEvalResults, EvaluationError> {
    debug!(
        "Starting LLM evaluation for {} records",
        dataset.records.len()
    );

    let join_set = match (
        config.embedder.as_ref(),
        config.embedding_targets.is_empty(),
    ) {
        (Some(embedder), false) => {
            debug!("Using embedding-enabled evaluation path");
            spawn_evaluation_tasks_with_embeddings(dataset, Arc::clone(embedder), config).await
        }
        _ => {
            debug!("Using standard evaluation path");
            spawn_evaluation_tasks_without_embeddings(dataset, config).await
        }
    };

    // Both paths now return the same type, so we can use a unified collection function
    let mut results = collect_and_align_results(join_set).await?;

    // Post-processing
    if config.needs_post_processing() {
        post_process_aligned_results(&mut results, config)?;
    }

    if config.compute_histograms {
        results.finalize(config)?;
    }

    Ok(results)
}

//#[pyfunction]
///// Function for evaluating LLM response and generating metrics.
///// The primary use case for evaluate_genai is to take a list of data samples, which often contain inputs and outputs
///// from LLM systems and evaluate them against user-defined metrics in a LLM as a judge pipeline. The user is expected provide
///// a list of dict objects and a list of LLMEval metrics. These eval metrics will be used to create a workflow, which is then
///// executed in an async context. All eval scores are extracted and returned to the user.
///// # Arguments
///// * `py`: The Python interpreter instance.
///// * `data`: A list of data samples to evaluate.
///// * `metrics`: A list of evaluation metrics to use.
//#[pyo3(signature = (records, metrics, config=None))]
//pub fn evaluate_genai(
//    records: Vec<GenAIEvalRecord>,
//    metrics: Vec<GenAIEvalMetric>,
//    config: Option<EvaluationConfig>,
//) -> Result<GenAIEvalResults, EvaluationError> {
//    let config = Arc::new(config.unwrap_or_default());
//
//    // Create runtime and execute evaluation pipeline
//    let mut results = app_state().handle().block_on(async {
//        let workflow = workflow_from_eval_metrics(metrics, "LLM Evaluation").await?;
//        async_evaluate_genai(workflow, records, &config).await
//    })?;
//
//    // Only run post-processing if needed
//    // Post processing includes calculating embedding means, similarities, clustering, and histograms
//    if config.needs_post_processing() {
//        results.finalize(&config)?;
//    }
//
//    Ok(results)
//}
//
