use crate::error::EvaluationError;
use crate::utils::{parse_embedder, post_process_aligned_results};
use ndarray::Array2;
use potato_head::{Embedder, PyHelperFuncs};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use scouter_profile::{Histogram, NumProfiler};
use scouter_types::genai::GenAIEvalSet;
use scouter_types::GenAIEvalRecord;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

pub fn array_to_dict<'py>(
    py: Python<'py>,
    array: &ArrayDataset,
) -> Result<Bound<'py, PyDict>, EvaluationError> {
    let pydict = PyDict::new(py);

    // set task ids
    pydict.set_item(
        "task",
        array.idx_map.values().cloned().collect::<Vec<String>>(),
    )?;

    // set feature columns
    for (i, feature) in array.feature_names.iter().enumerate() {
        let column_data: Vec<f64> = array.data.column(i).to_vec();
        pydict.set_item(feature, column_data)?;
    }

    // add cluster column if available
    if array.clusters.len() == array.data.nrows() {
        pydict.set_item("cluster", array.clusters.clone())?;
    }
    Ok(pydict)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[pyclass]
pub struct ClusterData {
    #[pyo3(get)]
    pub x: Vec<f64>,
    #[pyo3(get)]
    pub y: Vec<f64>,
    #[pyo3(get)]
    pub clusters: Vec<i32>,
    pub idx_map: HashMap<usize, String>,
}

impl ClusterData {
    pub fn new(
        x: Vec<f64>,
        y: Vec<f64>,
        clusters: Vec<i32>,
        idx_map: HashMap<usize, String>,
    ) -> Self {
        ClusterData {
            x,
            y,
            clusters,
            idx_map,
        }
    }
}

#[derive(Debug)]
pub struct ArrayDataset {
    pub data: Array2<f64>,
    pub feature_names: Vec<String>,
    pub idx_map: HashMap<usize, String>,
    pub clusters: Vec<i32>,
}

impl Default for ArrayDataset {
    fn default() -> Self {
        Self::new()
    }
}

impl ArrayDataset {
    pub fn new() -> Self {
        Self {
            data: Array2::zeros((0, 0)),
            feature_names: Vec::new(),
            idx_map: HashMap::new(),
            clusters: vec![],
        }
    }

    /// Build feature names from aligned results
    /// This extracts all unique task names, embedding targets, and similarity pairs
    /// from the evaluation results to create column names for the array dataset
    fn build_feature_names(results: &GenAIEvalResults) -> Result<Vec<String>, EvaluationError> {
        // Get first successful result to determine schema
        let first_result = results
            .aligned_results
            .iter()
            .find(|r| r.success)
            .ok_or(EvaluationError::NoResultsFound)?;

        let mut names = Vec::new();

        for task_record in &first_result.eval_set.records {
            names.push(task_record.task_id.clone());
        }

        names.extend(first_result.mean_embeddings.keys().cloned());
        names.extend(first_result.similarity_scores.keys().cloned());

        Ok(names)
    }

    // Build array dataset from aligned evaluation results
    /// Creates a 2D array where:
    /// - Rows = evaluation records
    /// - Columns = task scores, embedding means, similarity scores
    fn from_results(results: &GenAIEvalResults) -> Result<Self, EvaluationError> {
        if results.aligned_results.is_empty() {
            return Ok(Self::new());
        }

        // Only include successful evaluations in the dataset
        let successful_results: Vec<&AlignedEvalResult> = results
            .aligned_results
            .iter()
            .filter(|r| r.success)
            .collect();

        if successful_results.is_empty() {
            return Err(EvaluationError::NoResultsFound);
        }

        let feature_names = Self::build_feature_names(results)?;
        let n_rows = successful_results.len();
        let n_cols = feature_names.len();

        let mut data = Vec::with_capacity(n_rows * n_cols);
        let mut idx_map = HashMap::new();

        // Build task score lookup for efficient access
        // This maps task_id -> score value for quick column population
        for (row_idx, aligned) in successful_results.iter().enumerate() {
            idx_map.insert(row_idx, aligned.record_uid.clone());

            // Build lookup map from task_id to value
            let task_scores: HashMap<String, f64> = aligned
                .eval_set
                .records
                .iter()
                .map(|task| (task.task_id.clone(), task.value))
                .collect();

            // Collect all values in correct column order
            let row: Vec<f64> = feature_names
                .iter()
                .map(|feature_name| {
                    // Try task scores first
                    if let Some(&score) = task_scores.get(feature_name) {
                        return score;
                    }

                    // Try embedding means
                    if let Some(&mean) = aligned.mean_embeddings.get(feature_name) {
                        return mean;
                    }

                    // Try similarity scores
                    if let Some(&sim) = aligned.similarity_scores.get(feature_name) {
                        return sim;
                    }

                    // Default for missing values
                    0.0
                })
                .collect();

            data.extend(row);
        }

        let array = Array2::from_shape_vec((n_rows, n_cols), data)?;

        Ok(Self {
            data: array,
            feature_names,
            idx_map,
            clusters: vec![],
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[pyclass]
pub struct AlignedEvalResult {
    #[pyo3(get)]
    pub record_uid: String,

    #[pyo3(get)]
    pub eval_set: GenAIEvalSet,

    #[pyo3(get)]
    #[serde(skip)]
    pub embeddings: BTreeMap<String, Vec<f32>>,

    #[pyo3(get)]
    pub mean_embeddings: BTreeMap<String, f64>,

    #[pyo3(get)]
    pub similarity_scores: BTreeMap<String, f64>,

    #[pyo3(get)]
    pub success: bool,

    #[pyo3(get)]
    pub error_message: Option<String>,

    #[serde(skip)]
    pub context_snapshot: Option<BTreeMap<String, serde_json::Value>>,
}

impl AlignedEvalResult {
    /// Create from successful evaluation
    pub fn from_success(
        record: &GenAIEvalRecord,
        eval_set: GenAIEvalSet,
        embeddings: BTreeMap<String, Vec<f32>>,
    ) -> Self {
        Self {
            record_uid: record.uid.clone(),
            eval_set,
            embeddings,
            mean_embeddings: BTreeMap::new(),
            similarity_scores: BTreeMap::new(),
            success: true,
            error_message: None,
            context_snapshot: None,
        }
    }

    /// Create from failed evaluation
    pub fn from_failure(record: &GenAIEvalRecord, error: String) -> Self {
        Self {
            record_uid: record.uid.clone(),
            eval_set: GenAIEvalSet::empty(),
            embeddings: BTreeMap::new(),
            mean_embeddings: BTreeMap::new(),
            similarity_scores: BTreeMap::new(),
            success: false,
            error_message: Some(error),
            context_snapshot: None,
        }
    }

    /// Capture context snapshot for dataframe export
    pub fn capture_context(&mut self, record: &GenAIEvalRecord) {
        if let serde_json::Value::Object(context_map) = &record.context {
            self.context_snapshot = Some(
                context_map
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            );
        }
    }

    /// Get flattened data for dataframe export
    pub fn to_flat_map(&self) -> BTreeMap<String, serde_json::Value> {
        let mut flat = BTreeMap::new();

        // Record metadata
        flat.insert("record_uid".to_string(), self.record_uid.clone().into());
        flat.insert("success".to_string(), self.success.into());

        if let Some(error) = &self.error_message {
            flat.insert("error".to_string(), error.clone().into());
        }

        // Workflow-level metrics
        flat.insert(
            "total_tasks".to_string(),
            self.eval_set.inner.total_tasks.into(),
        );
        flat.insert(
            "passed_tasks".to_string(),
            self.eval_set.inner.passed_tasks.into(),
        );
        flat.insert(
            "failed_tasks".to_string(),
            self.eval_set.inner.failed_tasks.into(),
        );
        flat.insert(
            "pass_rate".to_string(),
            self.eval_set.inner.pass_rate.into(),
        );
        flat.insert(
            "duration_ms".to_string(),
            self.eval_set.inner.duration_ms.into(),
        );

        // Context from snapshot (if captured)
        if let Some(context) = &self.context_snapshot {
            for (key, value) in context {
                flat.insert(format!("context_{}", key), value.clone());
            }
        }

        // Task results
        for task_result in &self.eval_set.records {
            let prefix = format!("task_{}", task_result.task_id);
            flat.insert(format!("{}_passed", prefix), task_result.passed.into());
            flat.insert(format!("{}_value", prefix), task_result.value.into());
            flat.insert(
                format!("{}_message", prefix),
                task_result.message.clone().into(),
            );
        }

        // Mean embeddings
        for (target, mean) in &self.mean_embeddings {
            flat.insert(format!("embedding_mean_{}", target), (*mean).into());
        }

        // Similarity scores
        for (pair, score) in &self.similarity_scores {
            flat.insert(format!("similarity_{}", pair), (*score).into());
        }

        flat
    }
}

/// Updated results container
#[derive(Debug, Serialize, Deserialize)]
#[pyclass]
pub struct GenAIEvalResults {
    /// Aligned results in original record order
    pub aligned_results: Vec<AlignedEvalResult>,

    /// Quick lookup by record UID
    #[serde(skip)]
    pub results_by_uid: BTreeMap<String, usize>,

    #[pyo3(get)]
    pub errored_tasks: Vec<String>,

    pub cluster_data: Option<ClusterData>,

    #[pyo3(get)]
    pub histograms: Option<HashMap<String, Histogram>>,

    #[serde(skip)]
    pub array_dataset: Option<ArrayDataset>,
}

#[pymethods]
impl GenAIEvalResults {
    /// Get result by record UID
    pub fn __getitem__(&self, key: &str) -> Result<AlignedEvalResult, EvaluationError> {
        self.results_by_uid
            .get(key)
            .and_then(|&idx| self.aligned_results.get(idx))
            .cloned()
            .ok_or_else(|| EvaluationError::MissingKeyError(key.to_string()))
    }

    #[getter]
    pub fn successful_count(&self) -> usize {
        self.aligned_results.iter().filter(|r| r.success).count()
    }

    #[getter]
    pub fn failed_count(&self) -> usize {
        self.aligned_results.iter().filter(|r| !r.success).count()
    }

    /// Export to dataframe format
    #[pyo3(signature = (polars=false))]
    pub fn to_dataframe<'py>(
        &mut self,
        py: Python<'py>,
        polars: bool,
    ) -> Result<Bound<'py, PyAny>, EvaluationError> {
        // Build list of flat records
        let records_list = self
            .aligned_results
            .iter()
            .map(|r| r.to_flat_map())
            .collect::<Vec<_>>();

        // Convert to Python dict format
        let py_records = PyDict::new(py);

        // Transpose data for columnar format
        if let Some(first_record) = records_list.first() {
            for key in first_record.keys() {
                let column: Vec<_> = records_list
                    .iter()
                    .map(|r| r.get(key).cloned().unwrap_or(Value::Null))
                    .collect();
                let py_col = pythonize::pythonize(py, &column)?;
                py_records.set_item(key, py_col)?;
            }
        }

        let module = if polars { "polars" } else { "pandas" };
        let df_module = py.import(module)?;
        let df_class = df_module.getattr("DataFrame")?;

        if polars {
            Ok(df_class.call1((py_records,))?)
        } else {
            Ok(df_class.call_method1("from_dict", (py_records,))?)
        }
    }

    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

impl GenAIEvalResults {
    pub fn new() -> Self {
        Self {
            aligned_results: Vec::new(),
            results_by_uid: BTreeMap::new(),
            errored_tasks: Vec::new(),
            array_dataset: None,
            cluster_data: None,
            histograms: None,
        }
    }

    /// Add a successful result
    pub fn add_success(
        &mut self,
        record: &GenAIEvalRecord,
        eval_set: GenAIEvalSet,
        embeddings: BTreeMap<String, Vec<f32>>,
    ) {
        let uid = record.uid.clone();
        let idx = self.aligned_results.len();

        self.aligned_results.push(AlignedEvalResult::from_success(
            record, eval_set, embeddings,
        ));

        self.results_by_uid.insert(uid, idx);
    }

    /// Add a failed result - only reference the record
    pub fn add_failure(&mut self, record: &GenAIEvalRecord, error: String) {
        let uid = record.uid.clone();
        let idx = self.aligned_results.len();

        self.aligned_results
            .push(AlignedEvalResult::from_failure(record, error));
        self.results_by_uid.insert(uid.clone(), idx);
        self.errored_tasks.push(uid);
    }

    /// Finalize the results by performing post-processing steps which includes:
    /// - Post-processing embeddings (if any)
    /// - Building the array dataset (if not already built)
    /// - Performing clustering and dimensionality reduction (if enabled) for visualization
    /// # Arguments
    /// * `config` - The evaluation configuration that dictates post-processing behavior
    /// # Returns
    /// * `Result<(), EvaluationError>` - Returns Ok(()) if successful, otherwise returns
    pub fn finalize(&mut self, config: &Arc<EvaluationConfig>) -> Result<(), EvaluationError> {
        // Post-process embeddings if needed
        if !config.embedding_targets.is_empty() {
            post_process_aligned_results(self, config)?;
        }

        if config.compute_histograms {
            self.build_array_dataset()?;

            // Compute histograms for all numeric fields
            if let Some(array_dataset) = &self.array_dataset {
                let profiler = NumProfiler::new();
                let histograms = profiler.compute_histogram(
                    &array_dataset.data.view(),
                    &array_dataset.feature_names,
                    &10,
                    false,
                )?;
                self.histograms = Some(histograms);
            }
        }

        Ok(())
    }

    /// Build an NDArray dataset from the result tasks
    fn build_array_dataset(&mut self) -> Result<(), EvaluationError> {
        if self.array_dataset.is_none() {
            self.array_dataset = Some(ArrayDataset::from_results(self)?);
        }
        Ok(())
    }
}

impl Default for GenAIEvalResults {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Default)]
#[pyclass]
pub struct EvaluationConfig {
    // optional embedder for embedding-based evaluations
    pub embedder: Option<Arc<Embedder>>,

    // fields in the record to generate embeddings for
    pub embedding_targets: Vec<String>,

    // this will compute similarities for all combinations of embeddings in the targets
    // e.g. if you have targets ["a", "b"], it will compute similarity between a-b
    pub compute_similarity: bool,

    // whether to run clustering for all scores, embeddings and similarities (if available)
    pub cluster: bool,

    // whether to compute histograms for all scores, embeddings and similarities (if available)
    pub compute_histograms: bool,
}

#[pymethods]
impl EvaluationConfig {
    #[new]
    #[pyo3(signature = (embedder=None, embedding_targets=None, compute_similarity=false, cluster=false, compute_histograms=false))]
    /// Creates a new EvaluationConfig instance.
    /// # Arguments
    /// * `embedder` - Optional reference to a PyEmbedder instance.
    /// * `embedding_targets` - Optional list of fields in the record to generate embeddings for.
    /// * `compute_similarity` - Whether to compute similarities between embeddings.
    /// * `cluster` - Whether to run clustering for all scores, embeddings and similarities (if available).
    /// * `compute_histograms` - Whether to compute histograms for all scores, embeddings and similarities (if available).
    /// # Returns
    /// A new EvaluationConfig instance.
    fn new(
        embedder: Option<&Bound<'_, PyAny>>,
        embedding_targets: Option<Vec<String>>,
        compute_similarity: bool,
        cluster: bool,
        compute_histograms: bool,
    ) -> Result<Self, EvaluationError> {
        let embedder = parse_embedder(embedder)?;
        let embedding_targets = embedding_targets.unwrap_or_default();

        Ok(Self {
            embedder,
            embedding_targets,
            compute_similarity,
            cluster,
            compute_histograms,
        })
    }

    pub fn needs_post_processing(&self) -> bool {
        !self.embedding_targets.is_empty() || self.cluster
    }
}
