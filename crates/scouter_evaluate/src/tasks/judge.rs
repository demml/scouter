use crate::error::EvaluationError;
use core::fmt::Debug;
use potato_head::prompt_types::{Prompt, ResponseType};
use pyo3::prelude::*;
use scouter_types::error::TypeError;
use scouter_types::genai::EvaluationTaskType;
use scouter_types::genai::{assertion_value_from_py, AssertionValue, ComparisonOperator};
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
    pub operator: ComparisonOperator,

    pub task_type: EvaluationTaskType,
}

#[pymethods]
impl LLMJudgeTask {
    /// Creates a new LLMJudgeTask
    /// # Examples
    /// ```python
    /// task = LLMJudgeTask(
    ///     name="Sentiment Analysis Judge",
    ///     prompt=prompt_object,
    ///     expected_value="Positive",
    ///     operator=ComparisonOperator.EQUALS
    /// )
    /// # Arguments
    /// * `name`: The name of the judge task
    /// * `prompt`: The prompt object to be used for evaluation
    /// * `expected_value`: The expected value for the judgement
    /// * `operator`: The comparison operator to use
    /// # Returns
    /// A new LLMJudgeTask object
    #[new]
    #[pyo3(signature = (name, prompt, expected_value, operator))]
    pub fn new(
        name: &str,
        prompt: Prompt,
        expected_value: &Bound<'_, PyAny>,
        operator: ComparisonOperator,
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
            operator,
            task_type: EvaluationTaskType::LLMJudge,
        })
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }
}
