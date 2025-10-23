use crate::error::TypeError;
use crate::PyHelperFuncs;
use core::fmt::Debug;
use potato_head::prompt::ResponseType;
use potato_head::Prompt;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
enum PathSegment {
    Field(String),
    Index(usize),
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
fn assertion_value_from_py(value: &Bound<'_, PyAny>) -> Result<AssertionValue, TypeError> {
    if let Ok(s) = value.extract::<String>() {
        Ok(AssertionValue::String(s))
    } else if let Ok(i) = value.extract::<i64>() {
        Ok(AssertionValue::Integer(i))
    } else if let Ok(f) = value.extract::<f64>() {
        Ok(AssertionValue::Number(f))
    } else if let Ok(b) = value.extract::<bool>() {
        Ok(AssertionValue::Boolean(b))
    } else if let Ok(list) = value.extract::<Vec<Bound<'_, PyAny>>>() {
        let converted_list: Result<Vec<_>, _> = list
            .iter()
            .map(|item| assertion_value_from_py(item))
            .collect();
        Ok(AssertionValue::List(converted_list?))
    } else {
        Err(TypeError::InvalidAssertionValueType)
    }
}

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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct LLMJudgeTask {
    #[pyo3(get, set)]
    pub name: String,

    #[pyo3(get)]
    pub prompt: Prompt,

    pub expected_value: AssertionValue,

    pub comparison: ComparisonOperator,
}

#[pymethods]
impl LLMJudgeTask {
    #[new]
    #[pyo3(signature = (name, prompt, expected_value, comparison))]
    pub fn new(
        name: &str,
        prompt: Prompt,
        expected_value: &Bound<'_, PyAny>,
        comparison: ComparisonOperator,
    ) -> Result<Self, TypeError> {
        let expected_value = assertion_value_from_py(expected_value)?;

        // assert that the response json schema is not empty
        if prompt.response_json_schema.is_none() {
            return Err(TypeError::EmptyJsonResponseSchema);
        }
        Ok(Self {
            name: name.to_lowercase(),
            prompt,
            expected_value,
            comparison,
        })
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }
}
