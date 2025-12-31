use crate::tasks::evaluator::AssertionEvaluator;
use crate::{error::EvaluationError, tasks::traits::EvaluationTask};
use pyo3::prelude::*;
use scouter_types::genai::{
    assertion_value_from_py, AssertionValue, ComparisonOperator, EvaluationContext,
    EvaluationTaskResult, EvaluationTaskType,
};
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

    #[pyo3(get, set)]
    pub description: Option<String>,

    pub task_type: EvaluationTaskType,
}

#[pymethods]
impl FieldAssertionTask {
    #[new]
    /// Creates a new FieldAssertionTask
    /// # Examples
    /// ```python
    ///
    /// # assumed passed context at runtime
    /// # context = {
    /// #     "response": {
    /// #         "user": {
    /// #             "age": 25
    /// #         }
    /// #     }
    /// # }
    ///
    /// task = FieldAssertionTask(
    ///     field_path="response.user.age",
    ///     operator=ComparisonOperator.GREATER_THAN,
    ///     expected_value=18,
    ///     description="Check if user is an adult"
    /// )
    ///
    /// # assumed passed context at runtime
    /// # context = {
    /// #     "user": {
    /// #         "age": 25
    /// #     }
    /// # }
    ///
    /// task = FieldAssertionTask(
    ///     field_path="user.age",
    ///     operator=ComparisonOperator.GREATER_THAN,
    ///     expected_value=18,
    ///     description="Check if user is an adult"
    /// )
    /// ```
    /// # Arguments
    /// * `field_path`: The path to the field to be asserted
    /// * `operator`: The comparison operator to use
    /// * `expected_value`: The expected value for the assertion
    /// * `description`: Optional description for the assertion
    /// # Returns
    /// A new FieldAssertionTask object
    #[pyo3(signature = (field_path, expected_value, operator, description=None))]
    pub fn new(
        field_path: String,
        expected_value: &Bound<'_, PyAny>,
        operator: ComparisonOperator,
        description: Option<String>,
    ) -> Result<Self, EvaluationError> {
        let expected_value = assertion_value_from_py(expected_value)?;
        Ok(Self {
            field_path,
            operator,
            expected_value,
            description,
            task_type: EvaluationTaskType::FieldAssertion,
        })
    }
}

impl EvaluationTask for FieldAssertionTask {
    fn execute(
        &self,
        context: &EvaluationContext,
    ) -> Result<EvaluationTaskResult, EvaluationError> {
        let result = AssertionEvaluator::evaluate_assertion(&context.context, &self)?;

        Ok(EvaluationTaskResult::AssertionResults(result))
    }
}
