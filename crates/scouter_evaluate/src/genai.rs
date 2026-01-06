use crate::error::EvaluationError;
use crate::types::{EvaluationConfig, GenAIEvalResults};
use crate::utils::{
    collect_and_align_results, post_process_aligned_results,
    spawn_evaluation_tasks_with_embeddings, spawn_evaluation_tasks_without_embeddings,
};
use pyo3::prelude::*;
use pyo3::types::PyList;
use scouter_state::app_state;
use scouter_types::genai::{AssertionTask, GenAIDriftConfig, GenAIEvalProfile, LLMJudgeTask};
use scouter_types::GenAIEvalRecord;
use std::sync::Arc;
use tracing::{debug, instrument};
/// Main orchestration function that decides which execution path to take
/// # Arguments
/// * `dataset`: The dataset containing records to evaluate.
/// * `embedding_targets`: Optional list of fields to embed.
#[instrument(skip_all)]
pub async fn evaluate_genai_dataset(
    dataset: &GenAIEvalDataset,
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
            spawn_evaluation_tasks_with_embeddings(dataset, embedder.clone(), config).await
        }
        _ => {
            debug!("Using standard evaluation path");
            spawn_evaluation_tasks_without_embeddings(dataset, config).await
        }
    };

    // Pass records Arc for zero-copy access during collection
    let mut results = collect_and_align_results(join_set, &dataset.records).await?;

    // Post-processing
    if config.needs_post_processing() {
        post_process_aligned_results(&mut results, config)?;
    }

    if config.compute_histograms {
        results.finalize(config)?;
    }

    Ok(results)
}

#[pyclass]
pub struct GenAIEvalDataset {
    pub records: Arc<Vec<GenAIEvalRecord>>,
    pub profile: Arc<GenAIEvalProfile>,
}

#[pymethods]
impl GenAIEvalDataset {
    #[new]
    #[pyo3(signature = (records, tasks))]
    pub fn new(
        records: Vec<GenAIEvalRecord>,
        tasks: &Bound<'_, PyList>,
    ) -> Result<Self, EvaluationError> {
        let profile = GenAIEvalProfile::new_py(GenAIDriftConfig::default(), tasks)?;

        Ok(Self {
            records: Arc::new(records),
            profile: Arc::new(profile),
        })
    }

    #[getter]
    pub fn records(&self) -> Vec<GenAIEvalRecord> {
        // Clone for Python - only when explicitly requested
        (*self.records).clone()
    }

    #[getter]
    pub fn llm_judge_tasks(&self) -> Vec<LLMJudgeTask> {
        self.profile.llm_judge_tasks.clone()
    }

    #[getter]
    pub fn assertion_tasks(&self) -> Vec<AssertionTask> {
        self.profile.assertion_tasks.clone()
    }

    fn evaluate(
        &self,
        config: Option<EvaluationConfig>,
    ) -> Result<GenAIEvalResults, EvaluationError> {
        // Create runtime and execute evaluation pipeline
        // default config if none provided
        let config = Arc::new(config.unwrap_or_default());
        app_state()
            .handle()
            .block_on(async { evaluate_genai_dataset(&self, &config).await })
    }
}
