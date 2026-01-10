use crate::error::EvaluationError;
use crate::evaluate::types::{EvaluationConfig, GenAIEvalResults};
use crate::utils::{
    collect_and_align_results, post_process_aligned_results,
    spawn_evaluation_tasks_with_embeddings, spawn_evaluation_tasks_without_embeddings,
};
use pyo3::prelude::*;
use pyo3::types::{PyList, PySlice};
use pyo3::IntoPyObjectExt;
use scouter_state::app_state;
use scouter_types::genai::{AssertionTask, GenAIDriftConfig, GenAIEvalProfile, LLMJudgeTask};
use scouter_types::GenAIEvalRecord;
use scouter_types::PyHelperFuncs;
use serde::{Deserialize, Serialize};
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
pub struct DatasetRecords {
    records: Arc<Vec<GenAIEvalRecord>>,
    index: usize,
}

#[pymethods]
impl DatasetRecords {
    pub fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    pub fn __next__(mut slf: PyRefMut<'_, Self>) -> Option<GenAIEvalRecord> {
        if slf.index < slf.records.len() {
            let record = slf.records[slf.index].clone();
            slf.index += 1;
            Some(record)
        } else {
            None
        }
    }

    fn __getitem__<'py>(
        &self,
        py: Python<'py>,
        index: &Bound<'py, PyAny>,
    ) -> Result<Bound<'py, PyAny>, EvaluationError> {
        if let Ok(i) = index.extract::<isize>() {
            let len = self.records.len() as isize;
            let actual_index = if i < 0 { len + i } else { i };

            if actual_index < 0 || actual_index >= len {
                return Err(EvaluationError::IndexOutOfBounds {
                    index: i,
                    length: self.records.len(),
                });
            }

            Ok(self.records[actual_index as usize]
                .clone()
                .into_bound_py_any(py)?)
        } else if let Ok(slice) = index.cast::<PySlice>() {
            let indices = slice.indices(self.records.len() as isize)?;
            let mut result = Vec::new();

            let mut i = indices.start;
            while (indices.step > 0 && i < indices.stop) || (indices.step < 0 && i > indices.stop) {
                result.push(self.records[i as usize].clone());
                i += indices.step;
            }

            Ok(result.into_bound_py_any(py)?)
        } else {
            Err(EvaluationError::IndexOrSliceExpected)
        }
    }

    fn __len__(&self) -> usize {
        self.records.len()
    }
}

#[pyclass]
#[derive(Debug, Serialize, Deserialize)]
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
    pub fn records(&self) -> DatasetRecords {
        DatasetRecords {
            records: Arc::clone(&self.records),
            index: 0,
        }
    }

    fn __iter__(slf: PyRef<'_, Self>) -> DatasetRecords {
        DatasetRecords {
            records: Arc::clone(&slf.records),
            index: 0,
        }
    }

    fn __len__(&self) -> usize {
        self.records.len()
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
