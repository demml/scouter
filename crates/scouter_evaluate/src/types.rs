use crate::error::EvaluationError;
use crate::util::{cluster, parse_embedder, post_process, reduce_dimensions};
use ndarray::Array2;
use potato_head::{create_uuid7, Embedder, PyHelperFuncs, Score};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3::IntoPyObjectExt;
use scouter_profile::{Histogram, NumProfiler};
use scouter_types::{is_pydantic_model, json_to_pyobject_value, pyobject_to_json};
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

/// Enhanced results collection that captures both successes and failures
#[derive(Debug, Serialize, Deserialize)]
#[pyclass]
pub struct LLMEvalResults {
    pub results: HashMap<String, LLMEvalTaskResult>,

    #[pyo3(get)]
    pub errored_tasks: Vec<String>,

    pub cluster_data: Option<ClusterData>,

    #[pyo3(get)]
    pub histograms: Option<HashMap<String, Histogram>>,

    #[serde(skip)]
    pub array_dataset: Option<ArrayDataset>,
}

#[pymethods]
impl LLMEvalResults {
    /// Get tasks for a specific record ID
    pub fn __getitem__(&self, key: &str) -> Result<LLMEvalTaskResult, EvaluationError> {
        match self.results.get(key) {
            Some(value) => Ok(value.clone()),
            None => Err(EvaluationError::MissingKeyError(key.to_string())),
        }
    }

    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__json__(self)
    }

    #[staticmethod]
    pub fn model_validate_json(json_string: String) -> Result<LLMEvalResults, EvaluationError> {
        // deserialize the string to a struct
        Ok(serde_json::from_str(&json_string)?)
    }

    #[pyo3(signature = (polars=false))]
    pub fn to_dataframe<'py>(
        &mut self,
        py: Python<'py>,
        polars: bool,
    ) -> Result<Bound<'py, PyAny>, EvaluationError> {
        if self.array_dataset.is_none() {
            self.build_array_dataset()?;
        }

        let dataset = self.array_dataset.as_ref().unwrap();
        let records = array_to_dict(py, dataset)?;

        let module = if polars { "polars" } else { "pandas" };

        let df_module = py.import(module)?;
        let df_class = df_module.getattr("DataFrame")?;

        if polars {
            Ok(df_class.call1((records,))?)
        } else {
            Ok(df_class.call_method1("from_dict", (records,))?)
        }
    }

    #[getter]
    pub fn cluster_data(&self) -> Option<ClusterData> {
        self.cluster_data.clone()
    }

    #[getter]
    pub fn successful_count(&self) -> usize {
        self.results.len()
    }

    #[getter]
    pub fn failed_count(&self) -> usize {
        self.errored_tasks.len()
    }
}

impl LLMEvalResults {
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
            post_process(self, config);
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

        // Build array dataset for clustering/dimensionality reduction
        if config.cluster {
            self.build_array_dataset()?;
            self.perform_clustering_and_reduction()?;
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

    /// Perform clustering and dimensionality reduction on the dataset
    fn perform_clustering_and_reduction(&mut self) -> Result<(), EvaluationError> {
        let dataset = self.array_dataset.as_mut().unwrap();

        // Skip if insufficient data
        if dataset.data.nrows() < 5 {
            tracing::warn!("Insufficient data for clustering (need at least 5 points)");
            return Ok(());
        }

        // Cluster the data
        let clusters = cluster(&dataset.data)?;
        dataset.clusters = clusters
            .iter()
            .map(|&a| match a {
                Some(v) => v as i32,
                None => -1,
            })
            .collect();

        // Reduce dimensions if we have enough data
        let reduced = reduce_dimensions(&dataset.data, &dataset.clusters)?;

        if reduced.ncols() >= 2 {
            let x = reduced.column(0).to_vec();
            let y = reduced.column(1).to_vec();

            self.cluster_data = Some(ClusterData::new(x, y, dataset.clusters.clone()));
        }

        Ok(())
    }
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
}

impl ClusterData {
    pub fn new(x: Vec<f64>, y: Vec<f64>, clusters: Vec<i32>) -> Self {
        ClusterData { x, y, clusters }
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

    /// Build feature names from the results keys
    /// This is used when constructing a dataframe from the results and when writing records
    /// to the server
    fn build_feature_names(results: &LLMEvalResults) -> Result<Vec<String>, EvaluationError> {
        let first_task = results
            .results
            .values()
            .next()
            .ok_or(EvaluationError::NoResultsFound)?;

        let mut names = Vec::new();

        // BTreeMap iteration is already sorted
        names.extend(first_task.metrics.keys().cloned());
        names.extend(first_task.mean_embeddings.keys().cloned());
        names.extend(first_task.similarity_scores.keys().cloned());

        Ok(names)
    }

    fn from_results(results: &LLMEvalResults) -> Result<Self, EvaluationError> {
        if results.results.is_empty() {
            return Ok(Self::new());
        }

        let feature_names = Self::build_feature_names(results)?;
        let n_rows = results.results.len();
        let n_cols = feature_names.len();

        let mut data = Vec::with_capacity(n_rows * n_cols);
        let mut idx_map = HashMap::new();

        // Build data matrix efficiently
        for (i, task) in results.results.values().enumerate() {
            idx_map.insert(i, task.id.clone());

            // Collect all values in correct order (metrics, embeddings, similarities)
            let row: Vec<f64> = feature_names
                .iter()
                .map(|name| {
                    if let Some(score) = task.metrics.get(name) {
                        score.score as f64
                    } else if let Some(&mean) = task.mean_embeddings.get(name) {
                        mean
                    } else if let Some(&sim) = task.similarity_scores.get(name) {
                        sim
                    } else {
                        0.0 // Default for missing values
                    }
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

impl LLMEvalResults {
    pub fn new() -> Self {
        Self {
            results: HashMap::new(),
            errored_tasks: Vec::new(),
            array_dataset: None,
            cluster_data: None,
            histograms: None,
        }
    }
}

impl Default for LLMEvalResults {
    fn default() -> Self {
        Self::new()
    }
}

/// Struct for collecting results from LLM evaluation tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[pyclass]
pub struct LLMEvalTaskResult {
    #[pyo3(get)]
    pub id: String,

    #[pyo3(get)]
    pub metrics: BTreeMap<String, Score>,

    #[pyo3(get)]
    #[serde(skip)]
    pub embedding: BTreeMap<String, Vec<f32>>,

    #[pyo3(get)]
    pub mean_embeddings: BTreeMap<String, f64>,

    #[pyo3(get)]
    pub similarity_scores: BTreeMap<String, f64>,
}

#[pymethods]
impl LLMEvalTaskResult {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }
}

impl LLMEvalTaskResult {
    pub fn new(
        id: String,
        metrics: BTreeMap<String, Score>,
        embedding: BTreeMap<String, Vec<f32>>,
    ) -> Self {
        Self {
            id,
            metrics,
            embedding,
            mean_embeddings: BTreeMap::new(),
            similarity_scores: BTreeMap::new(),
        }
    }
}

#[pyclass]
#[derive(Clone, Debug)]
pub struct LLMEvalRecord {
    pub id: String,
    pub context: Value,
}

#[pymethods]
impl LLMEvalRecord {
    #[new]
    #[pyo3(signature = (
        context,
        id=None
    ))]

    /// Creates a new LLMRecord instance.
    /// The context is either a python dictionary or a pydantic basemodel.
    pub fn new(
        py: Python<'_>,
        context: Bound<'_, PyAny>,
        id: Option<String>,
    ) -> Result<Self, EvaluationError> {
        // check if context is a PyDict or PyObject(Pydantic model)
        let context_val = if context.is_instance_of::<PyDict>() {
            pyobject_to_json(&context)?
        } else if is_pydantic_model(py, &context)? {
            // Dump pydantic model to dictionary
            let model = context.call_method0("model_dump")?;

            // Serialize the dictionary to JSON
            pyobject_to_json(&model)?
        } else {
            Err(EvaluationError::MustBeDictOrBaseModel)?
        };

        let id = id.unwrap_or_else(create_uuid7);

        Ok(LLMEvalRecord {
            id,
            context: context_val,
        })
    }

    #[getter]
    pub fn context<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyAny>, EvaluationError> {
        Ok(json_to_pyobject_value(py, &self.context)?
            .into_bound_py_any(py)?
            .clone())
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
