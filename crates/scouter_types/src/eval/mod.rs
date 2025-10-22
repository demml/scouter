use crate::error::TypeError;
use crate::PyHelperFuncs;
use core::fmt::Debug;
use potato_head::prompt::ResponseType;
use potato_head::Prompt;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct LLMEvalMetric {
    #[pyo3(get, set)]
    pub name: String,

    #[pyo3(get)]
    pub prompt: Prompt,
}

#[pymethods]
impl LLMEvalMetric {
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
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AssertionValue {
    String(String),
    Number(f64),
    Integer(i64),
    Boolean(bool),
    List(Vec<AssertionValue>),
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldAssertion {
    #[pyo3(get, set)]
    pub field_path: String, // allows JSON-like syntax: "items", "user.name", "items[0]"

    #[pyo3(get, set)]
    pub operator: ComparisonOperator,

    #[pyo3(get, set)]
    pub expected_value: AssertionValue,

    #[pyo3(get, set)]
    pub description: Option<String>,
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
pub struct AssertionSuite {
    #[pyo3(get, set)]
    pub name: String,

    #[pyo3(get, set)]
    pub assertions: Vec<FieldAssertion>,
}
