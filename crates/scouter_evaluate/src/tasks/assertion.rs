use crate::error::EvaluationError;
use crate::types::{
    assertion_value_from_py, AssertionValue, ComparisonOperator, EvaluationTaskType,
};
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldAssertionTask {
    #[pyo3(get, set)]
    pub field_path: String,

    #[pyo3(get, set)]
    pub operator: ComparisonOperator,

    #[pyo3(get, set)]
    pub expected_value: AssertionValue,

    pub description: Option<String>,

    pub task_type: EvaluationTaskType,
}

#[pymethods]
impl FieldAssertionTask {
    pub fn description(&mut self, desc: String) {
        self.description = Some(desc);
    }

    #[getter]
    pub fn get_description(&self) -> Option<String> {
        self.description.clone()
    }
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionResult {
    #[pyo3(get)]
    pub passed: bool,

    #[pyo3(get)]
    pub field_path: String,

    #[pyo3(get)]
    pub expected: String,

    #[pyo3(get)]
    pub actual: String,

    #[pyo3(get)]
    pub message: String,
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionTasks {
    #[pyo3(get, set)]
    pub name: String,

    #[pyo3(get, set)]
    pub tasks: Vec<FieldAssertionTask>,
}

#[pyclass]
#[derive(Debug)]
pub struct Field {
    pub field_path: String,
}

#[pymethods]
impl Field {
    #[new]
    pub fn new(field_path: String) -> Self {
        Self { field_path }
    }

    /// General assertion method for creating a field-specific assertion
    /// # Arguments
    /// * `comparison`: The comparison operator to use
    /// * `value`: The expected value for the assertion
    /// * `description`: Optional description for the assertion
    /// Returns a FieldAssertionTask object
    /// # Errors
    /// Returns EvaluationError if the expected value cannot be converted
    pub fn assert(
        &self,
        comparison: ComparisonOperator,
        value: &Bound<'_, PyAny>,
        description: Option<String>,
    ) -> Result<FieldAssertionTask, EvaluationError> {
        Ok(FieldAssertionTask {
            field_path: self.field_path.clone(),
            operator: comparison,
            expected_value: assertion_value_from_py(value)?,
            description,
            task_type: EvaluationTaskType::FieldAssertion,
        })
    }
}
