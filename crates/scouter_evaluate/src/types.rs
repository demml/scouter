use crate::error::EvaluationError;
use potato_head::{create_uuid7, PyHelperFuncs};
use potato_head::{Score, Workflow};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3::IntoPyObjectExt;
use scouter_types::{is_pydantic_model, json_to_pyobject_value, pyobject_to_json};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Serialize)]
#[pyclass]
pub struct EvalResult {
    #[pyo3(get)]
    pub error: Option<String>,

    #[pyo3(get)]
    pub tasks: HashMap<String, Score>,

    #[pyo3(get)]
    pub id: String,
}

#[pymethods]
impl EvalResult {
    pub fn __getitem__(&self, key: &str) -> Result<Score, EvaluationError> {
        match self.tasks.get(key) {
            Some(value) => Ok(value.clone()),
            None => Err(EvaluationError::MissingKeyError(key.to_string())),
        }
    }
}

impl EvalResult {
    // Flattens the tasks hashmap into a list of dictionaries
    pub fn to_list<'py>(
        &self,
        py: Python<'py>,
    ) -> Result<Vec<Bound<'py, PyDict>>, EvaluationError> {
        let mut list = Vec::new();
        for (task_name, score) in &self.tasks {
            let dict = PyDict::new(py);
            dict.set_item("id", self.id.clone())?;
            dict.set_item("task_name", task_name.clone())?;
            dict.set_item("score", score.score)?;
            dict.set_item("reason", score.reason.clone())?;
            dict.set_item("error", self.error.clone())?;
            list.push(dict);
        }
        Ok(list)
    }
}

#[pyclass]
pub struct EvalResultIter {
    inner: std::vec::IntoIter<EvalResult>,
}

#[pymethods]
impl EvalResultIter {
    pub fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    pub fn __next__(mut slf: PyRefMut<'_, Self>) -> Option<EvalResult> {
        slf.inner.next()
    }
}

/// Enhanced results collection that captures both successes and failures
#[derive(Debug, Serialize)]
#[pyclass]
pub struct LLMEvalResults {
    pub results: HashMap<String, EvalResult>,
    pub errors: HashMap<String, String>,
    pub embedding_stats: Option<HashMap<String, f32>>,
}

#[pymethods]
impl LLMEvalResults {
    #[getter]
    pub fn successful_count(&self) -> usize {
        self.results.len()
    }

    #[getter]
    pub fn failed_count(&self) -> usize {
        self.errors.len()
    }

    #[getter]
    pub fn total_count(&self) -> usize {
        self.results.len() + self.errors.len()
    }

    pub fn get_errors(&self) -> HashMap<String, String> {
        self.errors.clone()
    }

    pub fn has_failures(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }

    /// Get tasks for a specific record ID
    pub fn __getitem__(&self, key: &str) -> Result<EvalResult, EvaluationError> {
        match self.results.get(key) {
            Some(value) => Ok(value.clone()),
            None => Err(EvaluationError::MissingKeyError(key.to_string())),
        }
    }

    pub fn __iter__(slf: PyRef<'_, Self>) -> Result<Py<EvalResultIter>, EvaluationError> {
        let iter = EvalResultIter {
            inner: slf
                .results
                .values()
                .cloned()
                .collect::<Vec<_>>()
                .into_iter(),
        };
        Ok(Py::new(slf.py(), iter)?)
    }

    #[pyo3(signature = (polars=false))]
    pub fn to_dataframe<'py>(
        &self,
        py: Python<'py>,
        polars: bool,
    ) -> Result<Bound<'py, PyAny>, EvaluationError> {
        let mut records = Vec::new();
        // columns: id, task name, score, reason, error

        for value in self.results.values() {
            let eval_list = value.to_list(py)?;
            // Flatten the list of dictionaries into a single vector
            records.extend(eval_list);
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
    pub fn new() -> Self {
        Self {
            results: HashMap::new(),
            errors: HashMap::new(),
            embedding_stats: None,
        }
    }

    pub fn add_successful_result(&mut self, id: String, result: EvalResult) {
        self.results.insert(id, result);
    }

    pub fn add_failed_result(&mut self, id: String, error: String) {
        self.errors.insert(id, error);
    }
}

/// Struct for collecting results from LLM evaluation tasks.
#[derive(Debug)]
pub struct LLMEvalTaskResult {
    pub id: String,
    pub workflow_result: Option<Arc<RwLock<Workflow>>>,
    pub embedding_results: Option<HashMap<String, f32>>,
    pub error: Option<String>,
}

impl LLMEvalTaskResult {
    pub fn success(
        id: String,
        workflow_result: Arc<RwLock<Workflow>>,
        embeddings: Option<HashMap<String, f32>>,
    ) -> Self {
        Self {
            id,
            workflow_result: Some(workflow_result),
            embedding_results: embeddings,
            error: None,
        }
    }

    pub fn failure(id: String, error: String, embeddings: Option<HashMap<String, f32>>) -> Self {
        Self {
            id,
            workflow_result: None,
            embedding_results: embeddings,
            error: Some(error),
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
