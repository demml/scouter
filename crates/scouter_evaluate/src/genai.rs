use crate::error::EvaluationError;
use crate::evaluate::types::{EvaluationConfig, GenAIEvalResults};
use crate::utils::{
    collect_and_align_results, post_process_aligned_results,
    spawn_evaluation_tasks_with_embeddings, spawn_evaluation_tasks_without_embeddings,
};
use pyo3::prelude::*;
use pyo3::types::PyList;
use scouter_state::app_state;
use scouter_types::genai::{AssertionTask, GenAIDriftConfig, GenAIEvalProfile, LLMJudgeTask};
use scouter_types::GenAIEvalRecord;
use scouter_types::PyHelperFuncs;
use serde::Serialize;
use std::collections::HashMap;
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

    let mut results = collect_and_align_results(join_set, &dataset.records).await?;

    if config.needs_post_processing() {
        post_process_aligned_results(&mut results, config)?;
    }

    if config.compute_histograms {
        results.finalize(config)?;
    }

    Ok(results)
}

#[pyclass]
#[derive(Debug, Serialize)]
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

    pub fn print_execution_plan(&self) -> Result<(), EvaluationError> {
        self.profile.print_execution_plan()?;
        Ok(())
    }

    #[pyo3(signature = (config=None))]
    fn evaluate(
        &self,
        config: Option<EvaluationConfig>,
    ) -> Result<GenAIEvalResults, EvaluationError> {
        let config = Arc::new(config.unwrap_or_default());
        app_state()
            .handle()
            .block_on(async { evaluate_genai_dataset(self, &config).await })
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }

    /// Update contexts by record ID mapping. This is the safest approach for
    /// ensuring context updates align with the correct records.
    ///
    /// # Arguments
    /// * `context_map` - Dictionary mapping record_id to new context object
    ///
    /// # Returns
    /// A new `GenAIEvalDataset` with updated contexts for matched IDs
    ///
    /// # Example
    /// ```python
    /// baseline_dataset = GenAIEvalDataset(records=[...], tasks=[...])
    ///
    /// # Update specific records by ID
    /// new_contexts = {
    ///     "product_classification_0": updated_context_0,
    ///     "product_classification_1": updated_context_1,
    /// }
    ///
    /// comparison_dataset = baseline_dataset.with_updated_contexts_by_id(new_contexts)
    /// ```
    #[pyo3(signature = (context_map))]
    pub fn with_updated_contexts_by_id(
        &self,
        py: Python<'_>,
        context_map: HashMap<String, Bound<'_, PyAny>>,
    ) -> Result<Self, EvaluationError> {
        let updated_records: Vec<GenAIEvalRecord> = self
            .records
            .iter()
            .map(|record| {
                if let Some(new_context) = context_map.get(&record.record_id) {
                    let mut updated_record = record.clone();
                    updated_record.update_context(py, new_context)?;
                    Ok(updated_record)
                } else {
                    Ok(record.clone())
                }
            })
            .collect::<Result<Vec<GenAIEvalRecord>, EvaluationError>>()?;

        Ok(Self {
            records: Arc::new(updated_records),
            profile: Arc::clone(&self.profile),
        })
    }
}
