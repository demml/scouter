use crate::error::EvaluationError;
use potato_head::Score;
use potato_head::{create_uuid7, PyHelperFuncs};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3::IntoPyObjectExt;
use scouter_types::{is_pydantic_model, json_to_pyobject_value, pyobject_to_json};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
#[pyclass]
pub struct MetricResult {
    #[pyo3(get)]
    pub task: String,

    pub score: Score,
}

#[pymethods]
impl MetricResult {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }

    #[getter]
    pub fn score(&self) -> i64 {
        self.score.score
    }

    #[getter]
    pub fn reason(&self) -> &str {
        &self.score.reason
    }
}

#[derive(Debug, Clone, Serialize)]
#[pyclass]
pub struct Embedding {
    #[pyo3(get)]
    pub field: String,

    #[pyo3(get)]
    pub value: Vec<f32>,
}

impl Embedding {
    pub fn new(field: String, value: Vec<f32>) -> Self {
        Self { field, value }
    }

    pub fn mean(&self) -> f32 {
        if self.value.is_empty() {
            0.0
        } else {
            let sum: f32 = self.value.iter().sum();
            sum / self.value.len() as f32
        }
    }
}

pub fn llm_tasks_to_records<'py>(
    py: Python<'py>,
    records: &mut Vec<Bound<'py, PyDict>>,
    task: &LLMEvalTaskResult,
    embedding_targets: &Option<Vec<String>>,
) -> Result<(), EvaluationError> {
    for metric in &task.metrics {
        let dict = PyDict::new(py);
        dict.set_item("id", &task.id)?;
        dict.set_item("task", metric.task.clone())?;
        dict.set_item("score", metric.score())?;
        dict.set_item("reason", metric.reason())?;

        // Iterate over embeddings if embedding targets are provided
        if let Some(embedding_targets) = &embedding_targets {
            for target in embedding_targets {
                if let Some(embedding) = task.embedding.iter().find(|e| &e.field == target) {
                    dict.set_item(target, embedding.mean())?;
                } else {
                    dict.set_item(target, None::<f32>)?;
                }
            }
        }
        records.push(dict);
    }

    Ok(())
}

/// Enhanced results collection that captures both successes and failures
#[derive(Debug, Serialize)]
#[pyclass]
pub struct LLMEvalResults {
    pub results: HashMap<String, LLMEvalTaskResult>,
    pub errored_tasks: Vec<String>,
    pub embedding_targets: Option<Vec<String>>,
}

#[pymethods]
impl LLMEvalResults {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }

    /// Get tasks for a specific record ID
    pub fn __getitem__(&self, key: &str) -> Result<LLMEvalTaskResult, EvaluationError> {
        match self.results.get(key) {
            Some(value) => Ok(value.clone()),
            None => Err(EvaluationError::MissingKeyError(key.to_string())),
        }
    }

    #[pyo3(signature = (polars=false))]
    pub fn to_dataframe<'py>(
        &self,
        py: Python<'py>,
        polars: bool,
    ) -> Result<Bound<'py, PyAny>, EvaluationError> {
        let mut records = Vec::new();

        for task in self.results.values() {
            llm_tasks_to_records(py, &mut records, task, &self.embedding_targets)?;
        }
        if polars {
            let polars = py.import("polars")?.getattr("DataFrame")?;
            let df = polars.call1((records,))?;
            Ok(df)
        } else {
            let pandas = py.import("pandas")?.getattr("DataFrame")?;
            let df = pandas.call_method1("from_records", (records,))?;

            Ok(df)
        }
    }
}

impl LLMEvalResults {
    pub fn new(embedding_targets: Option<Vec<String>>) -> Self {
        Self {
            results: HashMap::new(),
            errored_tasks: Vec::new(),
            embedding_targets,
        }
    }
}

/// Struct for collecting results from LLM evaluation tasks.
#[derive(Debug, Clone, Serialize)]
#[pyclass]
pub struct LLMEvalTaskResult {
    #[pyo3(get)]
    pub id: String,
    #[pyo3(get)]
    pub metrics: Vec<MetricResult>,
    #[pyo3(get)]
    pub embedding: Vec<Embedding>,
}

#[pymethods]
impl LLMEvalTaskResult {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }

    pub fn __getitem__(&self, key: &str) -> Result<MetricResult, EvaluationError> {
        match self.metrics.iter().find(|m| m.task == key) {
            Some(value) => Ok(value.clone()),
            None => Err(EvaluationError::MissingKeyError(key.to_string())),
        }
    }
}

impl LLMEvalTaskResult {
    pub fn new(id: String, metrics: Vec<MetricResult>, embedding: Vec<Embedding>) -> Self {
        Self {
            id,
            metrics,
            embedding,
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
