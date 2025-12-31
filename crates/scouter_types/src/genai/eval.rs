use crate::error::TypeError;
use crate::PyHelperFuncs;
use core::fmt::Debug;
use potato_head::prompt_types::{Prompt, ResponseType};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyFloat, PyInt, PyList, PyString};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct GenAIEvalMetric {
    #[pyo3(get, set)]
    pub name: String,

    #[pyo3(get)]
    pub prompt: Prompt,
}

#[pymethods]
impl GenAIEvalMetric {
    #[new]
    #[pyo3(signature = (name, prompt))]
    pub fn new(name: &str, prompt: Prompt) -> Result<Self, TypeError> {
        // assert that the prompt is a scoring prompt
        if prompt.response_type != ResponseType::Score {
            return Err(TypeError::InvalidResponseType);
        }
        Ok(Self {
            name: name.to_lowercase(),
            prompt,
        })
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ComparisonOperator {
    Equal,
    NotEqual,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    Contains,
    NotContains,
    StartsWith,
    EndsWith,
    Matches,
    HasLength,
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AssertionValue {
    String(String),
    Number(f64),
    Integer(i64),
    Boolean(bool),
    List(Vec<AssertionValue>),
    Null(),
}

impl AssertionValue {
    pub fn to_actual(self, comparison: &ComparisonOperator) -> AssertionValue {
        match comparison {
            ComparisonOperator::HasLength => match self {
                AssertionValue::List(arr) => AssertionValue::Integer(arr.len() as i64),
                AssertionValue::String(s) => AssertionValue::Integer(s.chars().count() as i64),
                _ => self,
            },
            _ => self,
        }
    }
}
/// Converts a PyAny value to an AssertionValue
///
/// # Errors
///
/// Returns `EvaluationError::UnsupportedType` if the Python type cannot be converted
/// to an `AssertionValue`.
pub fn assertion_value_from_py(value: &Bound<'_, PyAny>) -> Result<AssertionValue, TypeError> {
    // Check None first as it's a common case
    if value.is_none() {
        return Ok(AssertionValue::Null());
    }

    // Check bool before int (bool is subclass of int in Python)
    if value.is_instance_of::<PyBool>() {
        return Ok(AssertionValue::Boolean(value.extract()?));
    }

    if value.is_instance_of::<PyString>() {
        return Ok(AssertionValue::String(value.extract()?));
    }

    if value.is_instance_of::<PyInt>() {
        return Ok(AssertionValue::Integer(value.extract()?));
    }

    if value.is_instance_of::<PyFloat>() {
        return Ok(AssertionValue::Number(value.extract()?));
    }

    if value.is_instance_of::<PyList>() {
        // For list, we need to iterate, so one downcast is fine
        let list = value.cast::<PyList>()?; // Safe: we just checked
        let assertion_list = list
            .iter()
            .map(|item| assertion_value_from_py(&item))
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(AssertionValue::List(assertion_list));
    }

    // Return error for unsupported types
    Err(TypeError::UnsupportedType(
        value.get_type().name()?.to_string(),
    ))
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EvaluationTaskType {
    FieldAssertion,
    LLMJudge,
    HumanValidation,
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

pub enum EvaluationTaskResult {
    AssertionResults(AssertionResult),
    // Future task result types can be added here
}

#[derive(Debug, Clone)]
pub struct EvaluationContext {
    /// Raw JSON output from LLM
    pub context: Value,

    /// Results from previous tasks (for dependent tasks)
    pub previous_results: HashMap<String, AssertionResult>,
}
