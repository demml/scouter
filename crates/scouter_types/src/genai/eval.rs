use crate::error::TypeError;
use crate::genai::traits::TaskAccessor;
use crate::PyHelperFuncs;
use core::fmt::Debug;
use potato_head::prompt_types::{Prompt, ResponseType};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyFloat, PyInt, PyList, PyString};
use pythonize::{depythonize, pythonize};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssertionTask {
    #[pyo3(get, set)]
    pub id: String,

    #[pyo3(get, set)]
    pub field_path: Option<String>,

    #[pyo3(get, set)]
    pub operator: ComparisonOperator,

    pub expected_value: Value,

    #[pyo3(get, set)]
    pub description: Option<String>,

    #[pyo3(get, set)]
    pub depends_on: Vec<String>,

    pub task_type: EvaluationTaskType,
}

#[pymethods]
impl AssertionTask {
    #[new]
    /// Creates a new AssertionTask
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
    /// task = AssertionTask(
    ///     id="Check User Age",
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
    /// task = AssertionTask(
    ///     id="Check User Age",
    ///     field_path="user.age",
    ///     operator=ComparisonOperator.GREATER_THAN,
    ///     expected_value=18,
    ///     description="Check if user is an adult"
    /// )
    ///
    ///  /// # assume non-map context at runtime
    /// # context = 25
    ///
    /// task = AssertionTask(
    ///     id="Check User Age",
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
    /// A new AssertionTask object
    #[pyo3(signature = (id, field_path, expected_value, operator, description=None, depends_on=None))]
    pub fn new(
        id: String,
        field_path: Option<String>,
        expected_value: &Bound<'_, PyAny>,
        operator: ComparisonOperator,
        description: Option<String>,
        depends_on: Option<Vec<String>>,
    ) -> Result<Self, TypeError> {
        let expected_value = depythonize(expected_value)?;
        Ok(Self {
            id: id.to_lowercase(),
            field_path,
            operator,
            expected_value,
            description,
            task_type: EvaluationTaskType::Assertion,
            depends_on: depends_on.unwrap_or_default(),
        })
    }

    #[getter]
    pub fn get_expected_value<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyAny>, TypeError> {
        let py_value = pythonize(py, &self.expected_value)?;
        Ok(py_value)
    }
}

impl TaskAccessor for AssertionTask {
    fn field_path(&self) -> Option<&str> {
        self.field_path.as_deref()
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn operator(&self) -> &ComparisonOperator {
        &self.operator
    }

    fn expected_value(&self) -> &Value {
        &self.expected_value
    }

    fn depends_on(&self) -> &[String] {
        &self.depends_on
    }
}

pub trait ValueExt {
    /// Convert value to length for HasLength comparisons
    fn to_length(&self) -> Option<i64>;

    /// Extract numeric value for comparison
    fn as_numeric(&self) -> Option<f64>;

    /// Check if value is truthy
    fn is_truthy(&self) -> bool;
}

impl ValueExt for Value {
    fn to_length(&self) -> Option<i64> {
        match self {
            Value::Array(arr) => Some(arr.len() as i64),
            Value::String(s) => Some(s.chars().count() as i64),
            Value::Object(obj) => Some(obj.len() as i64),
            _ => None,
        }
    }

    fn as_numeric(&self) -> Option<f64> {
        match self {
            Value::Number(n) => n.as_f64(),
            _ => None,
        }
    }

    fn is_truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Number(n) => n.as_f64() != Some(0.0),
            Value::String(s) => !s.is_empty(),
            Value::Array(arr) => !arr.is_empty(),
            Value::Object(obj) => !obj.is_empty(),
        }
    }
}

/// Primary class for defining an LLM as a Judge in evaluation workflows
#[pyclass]
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct LLMJudgeTask {
    #[pyo3(get, set)]
    pub id: String,

    #[pyo3(get)]
    pub prompt: Prompt,

    #[pyo3(get)]
    pub field_path: Option<String>,

    pub expected_value: Value,

    #[pyo3(get)]
    pub operator: ComparisonOperator,

    pub task_type: EvaluationTaskType,

    #[pyo3(get, set)]
    pub depends_on: Vec<String>,

    #[pyo3(get, set)]
    pub max_retries: Option<u32>,
}

#[pymethods]
impl LLMJudgeTask {
    /// Creates a new LLMJudgeTask
    /// # Examples
    /// ```python
    /// task = LLMJudgeTask(
    ///     id="Sentiment Analysis Judge",
    ///     prompt=prompt_object,
    ///     expected_value="Positive",
    ///     operator=ComparisonOperator.EQUALS
    /// )
    /// # Arguments
    /// * `id: The id of the judge task
    /// * `prompt`: The prompt object to be used for evaluation
    /// * `expected_value`: The expected value for the judgement
    /// * `field_path`: Optional field path to extract from the context for evaluation
    /// * `operator`: The comparison operator to use
    /// * `depends_on`: Optional list of task IDs this task depends on
    /// * `max_retries`: Optional maximum number of retries for this task (defaults to 3 if not provided)
    /// # Returns
    /// A new LLMJudgeTask object
    #[new]
    #[pyo3(signature = (id, prompt, expected_value,  field_path,operator, depends_on=None, max_retries=None))]
    pub fn new(
        id: &str,
        prompt: Prompt,
        expected_value: &Bound<'_, PyAny>,
        field_path: Option<String>,
        operator: ComparisonOperator,
        depends_on: Option<Vec<String>>,
        max_retries: Option<u32>,
    ) -> Result<Self, TypeError> {
        let expected_value = depythonize(expected_value)?;

        // Prompt must have a response type of Score
        if prompt.response_type != ResponseType::Score
            || prompt.response_type != ResponseType::Pydantic
        {
            return Err(TypeError::InvalidResponseType);
        }

        Ok(Self {
            id: id.to_lowercase(),
            prompt,
            expected_value,
            operator,
            task_type: EvaluationTaskType::LLMJudge,
            depends_on: depends_on.unwrap_or_default(),
            max_retries: max_retries.or(Some(3)),
            field_path,
        })
    }

    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }

    #[getter]
    pub fn get_expected_value<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyAny>, TypeError> {
        let py_value = pythonize(py, &self.expected_value)?;
        Ok(py_value)
    }
}

impl TaskAccessor for LLMJudgeTask {
    fn field_path(&self) -> Option<&str> {
        self.field_path.as_deref()
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn operator(&self) -> &ComparisonOperator {
        &self.operator
    }

    fn expected_value(&self) -> &Value {
        &self.expected_value
    }
    fn depends_on(&self) -> &[String] {
        &self.depends_on
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

    pub fn to_serde_value(&self) -> Value {
        match self {
            AssertionValue::String(s) => Value::String(s.clone()),
            AssertionValue::Number(n) => Value::Number(serde_json::Number::from_f64(*n).unwrap()),
            AssertionValue::Integer(i) => Value::Number(serde_json::Number::from(*i)),
            AssertionValue::Boolean(b) => Value::Bool(*b),
            AssertionValue::List(arr) => {
                let json_arr: Vec<Value> = arr.iter().map(|v| v.to_serde_value()).collect();
                Value::Array(json_arr)
            }
            AssertionValue::Null() => Value::Null,
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
    Assertion,
    LLMJudge,
    HumanValidation,
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionResult {
    #[pyo3(get)]
    pub id: String,

    #[pyo3(get)]
    pub passed: bool,

    #[pyo3(get)]
    pub field_path: Option<String>,

    pub expected: Value,

    pub actual: Value,

    #[pyo3(get)]
    pub message: String,
}

#[pymethods]
impl AssertionResult {
    pub fn __str__(&self) -> String {
        // serialize the struct to a string
        PyHelperFuncs::__str__(self)
    }

    #[getter]
    pub fn get_expected<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyAny>, TypeError> {
        let py_value = pythonize(py, &self.expected)?;
        Ok(py_value)
    }

    #[getter]
    pub fn get_actual<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyAny>, TypeError> {
        let py_value = pythonize(py, &self.actual)?;
        Ok(py_value)
    }
}

impl AssertionResult {
    /// Convert to a metric value (1.0 for pass, 0.0 for fail)
    pub fn to_metric_value(&self) -> f64 {
        if self.passed {
            1.0
        } else {
            0.0
        }
    }
}

#[derive(Debug, Clone)]
pub struct EvaluationContext {
    /// Raw JSON output from LLM
    pub context: Value,

    pub task_results: HashMap<String, Value>,
}

impl EvaluationContext {
    /// Create a new evaluation context
    pub fn new(context: Value) -> Self {
        Self {
            context,
            task_results: HashMap::new(),
        }
    }

    pub fn build_merged_context(&self, depends_on: &[String]) -> Result<Value, TypeError> {
        self.build_dependency_context(depends_on)
    }

    /// Build context from dependent task results
    /// If only one dependency, return that result directly
    /// If multiple dependencies, return an object with each dependency's result
    /// keyed by the dependency ID
    /// # Arguments
    /// * `task`: The assertion task for which to build the dependency context
    /// # Returns
    /// A serde_json::Value representing the merged dependency context
    fn build_dependency_context(&self, depends_on: &[String]) -> Result<Value, TypeError> {
        if depends_on.len() == 1 {
            let dep_id = &depends_on[0];
            return self
                .task_results
                .get(dep_id)
                .cloned()
                .ok_or_else(|| TypeError::MissingDependency(dep_id.clone()));
        }

        let mut context_map = serde_json::Map::with_capacity(depends_on.len());
        for dep_id in depends_on {
            let dep_value = self
                .task_results
                .get(dep_id)
                .ok_or_else(|| TypeError::MissingDependency(dep_id.clone()))?;
            context_map.insert(dep_id.clone(), dep_value.clone());
        }

        Ok(Value::Object(context_map))
    }
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalTaskResult {
    #[pyo3(get)]
    pub task_id: String,

    #[pyo3(get)]
    pub task_type: EvaluationTaskType,

    #[pyo3(get)]
    pub passed: bool,

    #[pyo3(get)]
    pub value: f64,

    #[pyo3(get)]
    pub field_path: Option<String>,

    #[pyo3(get)]
    pub expected: String,

    #[pyo3(get)]
    pub actual: String,

    #[pyo3(get)]
    pub message: Option<String>,
}
