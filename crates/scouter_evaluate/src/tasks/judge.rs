use crate::error::EvaluationError;
use crate::types::EvaluationTaskType;
use crate::types::{assertion_value_from_py, AssertionValue, ComparisonOperator};
use core::fmt::Debug;
use potato_head::prompt::ResponseType;
use potato_head::Prompt;
use pyo3::prelude::*;
use scouter_types::error::TypeError;
use scouter_types::PyHelperFuncs;
use serde::{Deserialize, Serialize};

#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct LLMJudgeTask {
    #[pyo3(get, set)]
    pub name: String,

    #[pyo3(get)]
    pub prompt: Prompt,

    #[pyo3(get)]
    pub expected_value: AssertionValue,

    #[pyo3(get)]
    pub comparison: ComparisonOperator,

    pub task_type: EvaluationTaskType,
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
    ) -> Result<Self, EvaluationError> {
        let expected_value = assertion_value_from_py(expected_value)?;

        // Prompt must have a response type of Score
        if prompt.response_type != ResponseType::Score {
            return Err(TypeError::InvalidResponseType.into());
        }
        Ok(Self {
            name: name.to_lowercase(),
            prompt,
            expected_value,
            comparison,
            task_type: EvaluationTaskType::LLMJudge,
        })
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }
}
