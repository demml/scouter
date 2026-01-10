use crate::error::EvaluationError;
use crate::evaluate::compare::compare_results;
use crate::utils::parse_embedder;
use crate::utils::post_process_aligned_results;
use ndarray::Array2;
use owo_colors::OwoColorize;
use potato_head::Embedder;
use potato_head::PyHelperFuncs;
use pyo3::prelude::*;
use pyo3::types::IntoPyDict;
use pyo3::types::PyDict;
use scouter_profile::{Histogram, NumProfiler};
use scouter_types::genai::GenAIEvalSet;
use scouter_types::{GenAIEvalRecord, TaskResultTableEntry, WorkflowResultTableEntry};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tabled::Tabled;
use tabled::{
    settings::{object::Rows, Alignment, Color, Format, Style},
    Table,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[pyclass]
pub struct MissingTask {
    #[pyo3(get)]
    pub task_id: String,

    #[pyo3(get)]
    pub present_in: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[pyclass]
pub struct TaskComparison {
    #[pyo3(get)]
    pub task_id: String,

    #[pyo3(get)]
    pub baseline_passed: bool,

    #[pyo3(get)]
    pub comparison_passed: bool,

    #[pyo3(get)]
    pub status_changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[pyclass]
pub struct WorkflowComparison {
    #[pyo3(get)]
    pub baseline_uid: String,

    #[pyo3(get)]
    pub comparison_uid: String,

    #[pyo3(get)]
    pub baseline_pass_rate: f64,

    #[pyo3(get)]
    pub comparison_pass_rate: f64,

    #[pyo3(get)]
    pub pass_rate_delta: f64,

    #[pyo3(get)]
    pub is_regression: bool,

    #[pyo3(get)]
    pub task_comparisons: Vec<TaskComparison>,
}

#[derive(Tabled)]
struct WorkflowComparisonEntry {
    #[tabled(rename = "Baseline UID")]
    baseline_uid: String,
    #[tabled(rename = "Comparison UID")]
    comparison_uid: String,
    #[tabled(rename = "Baseline Pass Rate")]
    baseline_pass_rate: String,
    #[tabled(rename = "Comparison Pass Rate")]
    comparison_pass_rate: String,
    #[tabled(rename = "Delta")]
    delta: String,
    #[tabled(rename = "Status")]
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[pyclass]
pub struct ComparisonResults {
    #[pyo3(get)]
    pub workflow_comparisons: Vec<WorkflowComparison>,

    #[pyo3(get)]
    pub total_workflows: usize,

    #[pyo3(get)]
    pub improved_workflows: usize,

    #[pyo3(get)]
    pub regressed_workflows: usize,

    #[pyo3(get)]
    pub unchanged_workflows: usize,

    #[pyo3(get)]
    pub mean_pass_rate_delta: f64,

    #[pyo3(get)]
    pub task_status_changes: Vec<TaskComparison>,

    #[pyo3(get)]
    pub missing_tasks: Vec<MissingTask>,

    #[pyo3(get)]
    pub baseline_workflow_count: usize,

    #[pyo3(get)]
    pub comparison_workflow_count: usize,
}

#[pymethods]
impl ComparisonResults {
    #[getter]
    pub fn has_missing_tasks(&self) -> bool {
        !self.missing_tasks.is_empty()
    }

    pub fn print_missing_tasks(&self) {
        if !self.missing_tasks.is_empty() {
            println!("\n{}", "⚠ Missing Tasks".yellow().bold());

            let baseline_only: Vec<_> = self
                .missing_tasks
                .iter()
                .filter(|t| t.present_in == "baseline_only")
                .collect();

            let comparison_only: Vec<_> = self
                .missing_tasks
                .iter()
                .filter(|t| t.present_in == "comparison_only")
                .collect();

            if !baseline_only.is_empty() {
                println!("  Baseline only ({} tasks):", baseline_only.len());
                for task in baseline_only {
                    println!("    - {}", task.task_id);
                }
            }

            if !comparison_only.is_empty() {
                println!("  Comparison only ({} tasks):", comparison_only.len());
                for task in comparison_only {
                    println!("    - {}", task.task_id);
                }
            }
        }
    }

    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }

    pub fn as_table(&self) {
        self.print_summary_table();

        if !self.task_status_changes.is_empty() {
            println!("\n{}", "Task Status Changes".truecolor(245, 77, 85).bold());
            self.print_status_changes_table();
        }

        if self.has_missing_tasks() {
            self.print_missing_tasks();
        }
    }

    fn print_summary_table(&self) {
        let entries: Vec<_> = self
            .workflow_comparisons
            .iter()
            .map(|wc| WorkflowComparisonEntry {
                baseline_uid: wc.baseline_uid.clone(),
                comparison_uid: wc.comparison_uid.clone(),
                baseline_pass_rate: format!("{:.2}%", wc.baseline_pass_rate * 100.0),
                comparison_pass_rate: format!("{:.2}%", wc.comparison_pass_rate * 100.0),
                delta: format!("{:+.2}%", wc.pass_rate_delta * 100.0),
                status: if wc.is_regression {
                    "Regressed".to_string()
                } else if wc.pass_rate_delta > 0.01 {
                    "Improved".to_string()
                } else {
                    "Unchanged".to_string()
                },
            })
            .collect();

        let mut table = Table::new(entries);
        table.with(Style::sharp());
        println!("{}", table);
    }

    fn print_status_changes_table(&self) {
        let entries: Vec<_> = self
            .task_status_changes
            .iter()
            .map(|tc| TaskStatusChangeEntry {
                task_id: tc.task_id.clone(),
                baseline_status: if tc.baseline_passed { "Pass" } else { "Fail" }.to_string(),
                comparison_status: if tc.comparison_passed { "Pass" } else { "Fail" }.to_string(),
                change: match (tc.baseline_passed, tc.comparison_passed) {
                    (true, false) => "Pass → Fail",
                    (false, true) => "Fail → Pass",
                    _ => "No Change",
                }
                .to_string(),
            })
            .collect();

        let mut table = Table::new(entries);
        table.with(Style::sharp());
        println!("{}", table);
    }
}

#[derive(Tabled)]
struct TaskStatusChangeEntry {
    #[tabled(rename = "Task ID")]
    task_id: String,
    #[tabled(rename = "Baseline")]
    baseline_status: String,
    #[tabled(rename = "Comparison")]
    comparison_status: String,
    #[tabled(rename = "Change")]
    change: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[pyclass]
pub struct GenAIEvalResults {
    /// Aligned results in original record order
    pub aligned_results: Vec<AlignedEvalResult>,

    /// Quick lookup by record UID
    #[serde(skip)]
    pub results_by_uid: BTreeMap<String, usize>,

    #[serde(skip)]
    pub results_by_id: BTreeMap<String, usize>,

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
        self.results_by_id
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
        let all_task_records: Vec<_> = self
            .aligned_results
            .iter()
            .flat_map(|r| r.to_flat_task_records())
            .collect();

        if all_task_records.is_empty() {
            return Err(EvaluationError::NoResultsFound);
        }

        let py_records = PyDict::new(py);

        // Collect all unique column names
        let mut all_columns = std::collections::BTreeSet::new();
        for record in &all_task_records {
            all_columns.extend(record.keys().cloned());
        }

        // Build columns in consistent order
        for column_name in all_columns {
            let column_data: Vec<_> = all_task_records
                .iter()
                .map(|record| record.get(&column_name).cloned().unwrap_or(Value::Null))
                .collect();

            let py_col = pythonize::pythonize(py, &column_data)?;
            py_records.set_item(&column_name, py_col)?;
        }

        let module = if polars { "polars" } else { "pandas" };
        let df_module = py.import(module)?;
        let df_class = df_module.getattr("DataFrame")?;

        if polars {
            let schema = self.get_schema_mapping(py)?;
            let schema_dict = &[("schema", schema)].into_py_dict(py)?;
            schema_dict.set_item("strict", false)?;
            Ok(df_class.call((py_records,), Some(schema_dict))?)
        } else {
            Ok(df_class.call_method1("from_dict", (py_records,))?)
        }
    }

    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }

    #[pyo3(signature = (show_tasks=false))]
    /// Display results as a table in the console
    /// # Arguments
    /// * `show_tasks` - If true, display detailed task results; otherwise, show workflow summary
    pub fn as_table(&self, show_tasks: bool) {
        if show_tasks {
            let tasks_table = self.build_tasks_table();
            println!("\n{}", "Task Details".truecolor(245, 77, 85).bold());
            println!("{}", tasks_table);
        } else {
            let workflow_table = self.build_workflow_table();
            println!("\n{}", "Workflow Summary".truecolor(245, 77, 85).bold());
            println!("{}", workflow_table);
        }
    }

    pub fn model_dump_json(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__json__(self)
    }

    #[staticmethod]
    pub fn model_validate_json(json_string: String) -> Result<Self, EvaluationError> {
        Ok(serde_json::from_str(&json_string)?)
    }

    /// Compare this evaluation against another baseline evaluation
    /// Matches workflows by their task structure and compares task-by-task
    #[pyo3(signature = (baseline, regression_threshold=0.05))]
    pub fn compare_to(
        &self,
        baseline: &GenAIEvalResults,
        regression_threshold: f64,
    ) -> Result<ComparisonResults, EvaluationError> {
        compare_results(baseline, self, regression_threshold)
    }
}

impl GenAIEvalResults {
    fn get_schema_mapping<'py>(
        &self,
        py: Python<'py>,
    ) -> Result<Bound<'py, PyDict>, EvaluationError> {
        let schema = PyDict::new(py);
        let pl = py.import("polars")?;

        schema.set_item("created_at", pl.getattr("Utf8")?)?;
        schema.set_item("record_uid", pl.getattr("Utf8")?)?;
        schema.set_item("success", pl.getattr("Boolean")?)?;
        schema.set_item("workflow_error", pl.getattr("Utf8")?)?;

        schema.set_item("workflow_total_tasks", pl.getattr("Int64")?)?;
        schema.set_item("workflow_passed_tasks", pl.getattr("Int64")?)?;
        schema.set_item("workflow_failed_tasks", pl.getattr("Int64")?)?;
        schema.set_item("workflow_pass_rate", pl.getattr("Float64")?)?;
        schema.set_item("workflow_duration_ms", pl.getattr("Int64")?)?;

        schema.set_item("task_id", pl.getattr("Utf8")?)?;
        schema.set_item("task_type", pl.getattr("Utf8")?)?;
        schema.set_item("task_passed", pl.getattr("Boolean")?)?;
        schema.set_item("task_value", pl.getattr("Float64")?)?;
        schema.set_item("task_message", pl.getattr("Utf8")?)?;
        schema.set_item("task_field_path", pl.getattr("Utf8")?)?;
        schema.set_item("task_operator", pl.getattr("Utf8")?)?;
        schema.set_item("task_expected", pl.getattr("Utf8")?)?;
        schema.set_item("task_actual", pl.getattr("Utf8")?)?;

        schema.set_item("context", pl.getattr("Utf8")?)?;
        schema.set_item("embedding_means", pl.getattr("Utf8")?)?;
        schema.set_item("similarity_scores", pl.getattr("Utf8")?)?;

        Ok(schema)
    }
    /// Build workflow result table for console display
    fn build_workflow_table(&self) -> Table {
        let entries: Vec<WorkflowResultTableEntry> = self
            .aligned_results
            .iter()
            .flat_map(|result| result.eval_set.build_workflow_entries())
            .collect();

        let mut table = Table::new(entries);
        table.with(Style::sharp());

        table.modify(
            Rows::new(0..1),
            (
                Format::content(|s: &str| s.truecolor(245, 77, 85).bold().to_string()),
                Alignment::center(),
                Color::BOLD,
            ),
        );
        table
    }

    /// Build detailed task results table for console display
    fn build_tasks_table(&self) -> Table {
        let entries: Vec<TaskResultTableEntry> = self
            .aligned_results
            .iter()
            .flat_map(|result| result.eval_set.build_task_entries())
            .collect();

        let mut table = Table::new(entries);
        table.with(Style::sharp());

        table.modify(
            Rows::new(0..1),
            (
                Format::content(|s: &str| s.truecolor(245, 77, 85).bold().to_string()),
                Alignment::center(),
                Color::BOLD,
            ),
        );

        table
    }

    pub fn new() -> Self {
        Self {
            aligned_results: Vec::new(),
            results_by_uid: BTreeMap::new(),
            results_by_id: BTreeMap::new(),
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

        // if record has an id, also index by that
        if !record.record_id.is_empty() {
            self.results_by_id.insert(record.record_id.clone(), idx);
        }
    }

    /// Add a failed result - only reference the record
    pub fn add_failure(&mut self, record: &GenAIEvalRecord, error: String) {
        let uid = record.uid.clone();
        let idx = self.aligned_results.len();

        self.aligned_results
            .push(AlignedEvalResult::from_failure(record, error));
        self.results_by_uid.insert(uid.clone(), idx);
        if !record.record_id.is_empty() {
            self.results_by_id.insert(record.record_id.clone(), idx);
        }
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
    pub fn from_results(results: &GenAIEvalResults) -> Result<Self, EvaluationError> {
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

#[pymethods]
impl AlignedEvalResult {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }

    #[getter]
    pub fn task_count(&self) -> usize {
        self.eval_set.records.len()
    }
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
    pub fn to_flat_task_records(&self) -> Vec<BTreeMap<String, serde_json::Value>> {
        let mut records = Vec::new();

        for task_result in &self.eval_set.records {
            let mut flat = BTreeMap::new();

            // Workflow metadata (repeated for each task)
            flat.insert(
                "created_at".to_string(),
                self.eval_set.inner.created_at.to_rfc3339().into(),
            );
            flat.insert("record_uid".to_string(), self.record_uid.clone().into());
            flat.insert("success".to_string(), self.success.into());

            // insert workflow error or "" if none
            flat.insert(
                "workflow_error".to_string(),
                match &self.error_message {
                    Some(err) => serde_json::Value::String(err.clone()),
                    None => serde_json::Value::String("".to_string()),
                },
            );

            // Workflow-level metrics
            flat.insert(
                "workflow_total_tasks".to_string(),
                self.eval_set.inner.total_tasks.into(),
            );
            flat.insert(
                "workflow_passed_tasks".to_string(),
                self.eval_set.inner.passed_tasks.into(),
            );
            flat.insert(
                "workflow_failed_tasks".to_string(),
                self.eval_set.inner.failed_tasks.into(),
            );
            flat.insert(
                "workflow_pass_rate".to_string(),
                self.eval_set.inner.pass_rate.into(),
            );
            flat.insert(
                "workflow_duration_ms".to_string(),
                self.eval_set.inner.duration_ms.into(),
            );

            // Task-specific data
            flat.insert("task_id".to_string(), task_result.task_id.clone().into());
            flat.insert(
                "task_type".to_string(),
                task_result.task_type.to_string().into(),
            );
            flat.insert("task_passed".to_string(), task_result.passed.into());
            flat.insert("task_value".to_string(), task_result.value.into());
            flat.insert(
                "task_message".to_string(),
                serde_json::Value::String(task_result.message.clone()),
            );

            flat.insert(
                "task_field_path".to_string(),
                match &task_result.field_path {
                    Some(path) => serde_json::Value::String(path.clone()),
                    None => serde_json::Value::Null,
                },
            );

            flat.insert(
                "task_operator".to_string(),
                task_result.operator.to_string().into(),
            );
            flat.insert("task_expected".to_string(), task_result.expected.clone());
            flat.insert("task_actual".to_string(), task_result.actual.clone());

            // Context as single JSON column
            flat.insert(
                "context".to_string(),
                self.context_snapshot
                    .as_ref()
                    .map(|ctx| serde_json::to_value(ctx).unwrap_or(serde_json::Value::Null))
                    .unwrap_or(serde_json::Value::Null),
            );

            // Embedding means as single JSON column
            flat.insert(
                "embedding_means".to_string(),
                serde_json::to_value(&self.mean_embeddings).unwrap_or(serde_json::Value::Null),
            );

            // Similarity scores as single JSON column
            flat.insert(
                "similarity_scores".to_string(),
                serde_json::to_value(&self.similarity_scores).unwrap_or(serde_json::Value::Null),
            );

            records.push(flat);
        }

        records
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

    // whether to compute histograms for all scores, embeddings and similarities (if available)
    pub compute_histograms: bool,
}

#[pymethods]
impl EvaluationConfig {
    #[new]
    #[pyo3(signature = (embedder=None, embedding_targets=None, compute_similarity=false, compute_histograms=false))]
    /// Creates a new EvaluationConfig instance.
    /// # Arguments
    /// * `embedder` - Optional reference to a PyEmbedder instance.
    /// * `embedding_targets` - Optional list of fields in the record to generate embeddings for.
    /// * `compute_similarity` - Whether to compute similarities between embeddings.
    /// * `compute_histograms` - Whether to compute histograms for all scores, embeddings and similarities (if available).
    /// # Returns
    /// A new EvaluationConfig instance.
    fn new(
        embedder: Option<&Bound<'_, PyAny>>,
        embedding_targets: Option<Vec<String>>,
        compute_similarity: bool,
        compute_histograms: bool,
    ) -> Result<Self, EvaluationError> {
        let embedder = parse_embedder(embedder)?;
        let embedding_targets = embedding_targets.unwrap_or_default();

        Ok(Self {
            embedder,
            embedding_targets,
            compute_similarity,
            compute_histograms,
        })
    }

    pub fn needs_post_processing(&self) -> bool {
        !self.embedding_targets.is_empty()
    }
}
