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
use std::fmt::Display;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssertionResult {
    pub passed: bool,
    pub actual: Value,
    pub message: String,
}

impl AssertionResult {
    pub fn new(passed: bool, actual: Value, message: String) -> Self {
        Self {
            passed,
            actual,
            message,
        }
    }
    pub fn to_metric_value(&self) -> f64 {
        if self.passed {
            1.0
        } else {
            0.0
        }
    }
}

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

    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<AssertionResult>,
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
            result: None,
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

    fn task_type(&self) -> &EvaluationTaskType {
        &self.task_type
    }

    fn expected_value(&self) -> &Value {
        &self.expected_value
    }

    fn depends_on(&self) -> &[String] {
        &self.depends_on
    }

    fn add_result(&mut self, result: AssertionResult) {
        self.result = Some(result);
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

    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<AssertionResult>,
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
            result: None,
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

impl LLMJudgeTask {
    /// Creates a new LLMJudgeTask with Rust types
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
    pub fn new_rs(
        id: &str,
        prompt: Prompt,
        expected_value: Value,
        field_path: Option<String>,
        operator: ComparisonOperator,
        depends_on: Option<Vec<String>>,
        max_retries: Option<u32>,
    ) -> Self {
        Self {
            id: id.to_lowercase(),
            prompt,
            expected_value,
            operator,
            task_type: EvaluationTaskType::LLMJudge,
            depends_on: depends_on.unwrap_or_default(),
            max_retries: max_retries.or(Some(3)),
            field_path,
            result: None,
        }
    }
}

impl TaskAccessor for LLMJudgeTask {
    fn field_path(&self) -> Option<&str> {
        self.field_path.as_deref()
    }

    fn id(&self) -> &str {
        &self.id
    }
    fn task_type(&self) -> &EvaluationTaskType {
        &self.task_type
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
    fn add_result(&mut self, result: AssertionResult) {
        self.result = Some(result);
    }
}

#[derive(Debug, Clone)]
pub enum EvaluationTask {
    Assertion(AssertionTask),
    LLMJudge(Box<LLMJudgeTask>),
}

impl TaskAccessor for EvaluationTask {
    fn field_path(&self) -> Option<&str> {
        match self {
            EvaluationTask::Assertion(t) => t.field_path(),
            EvaluationTask::LLMJudge(t) => t.field_path(),
        }
    }

    fn id(&self) -> &str {
        match self {
            EvaluationTask::Assertion(t) => t.id(),
            EvaluationTask::LLMJudge(t) => t.id(),
        }
    }

    fn task_type(&self) -> &EvaluationTaskType {
        match self {
            EvaluationTask::Assertion(t) => t.task_type(),
            EvaluationTask::LLMJudge(t) => t.task_type(),
        }
    }

    fn operator(&self) -> &ComparisonOperator {
        match self {
            EvaluationTask::Assertion(t) => t.operator(),
            EvaluationTask::LLMJudge(t) => t.operator(),
        }
    }

    fn expected_value(&self) -> &Value {
        match self {
            EvaluationTask::Assertion(t) => t.expected_value(),
            EvaluationTask::LLMJudge(t) => t.expected_value(),
        }
    }

    fn depends_on(&self) -> &[String] {
        match self {
            EvaluationTask::Assertion(t) => t.depends_on(),
            EvaluationTask::LLMJudge(t) => t.depends_on(),
        }
    }

    fn add_result(&mut self, result: AssertionResult) {
        match self {
            EvaluationTask::Assertion(t) => t.add_result(result),
            EvaluationTask::LLMJudge(t) => t.add_result(result),
        }
    }
}

pub struct EvaluationTasks(Vec<EvaluationTask>);

impl EvaluationTasks {
    /// Creates a new empty builder
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Generic method that accepts anything implementing Into<EvaluationTask>
    pub fn add_task(mut self, task: impl Into<EvaluationTask>) -> Self {
        self.0.push(task.into());
        self
    }

    /// Builds and returns the Vec<EvaluationTask>
    pub fn build(self) -> Vec<EvaluationTask> {
        self.0
    }
}

// Implement From trait for automatic conversion
impl From<AssertionTask> for EvaluationTask {
    fn from(task: AssertionTask) -> Self {
        EvaluationTask::Assertion(task)
    }
}

impl From<LLMJudgeTask> for EvaluationTask {
    fn from(task: LLMJudgeTask) -> Self {
        EvaluationTask::LLMJudge(Box::new(task))
    }
}

impl Default for EvaluationTasks {
    fn default() -> Self {
        Self::new()
    }
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ComparisonOperator {
    // Existing operators
    Equals,
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
    HasLengthGreaterThan,
    HasLengthLessThan,
    HasLengthEqual,
    HasLengthGreaterThanOrEqual,
    HasLengthLessThanOrEqual,

    // Type Validation Operators
    IsNumeric,
    IsString,
    IsBoolean,
    IsNull,
    IsArray,
    IsObject,

    // Pattern & Format Validators
    IsEmail,
    IsUrl,
    IsUuid,
    IsIso8601,
    IsJson,
    MatchesRegex,

    // Numeric Range Operators
    InRange,
    NotInRange,
    IsPositive,
    IsNegative,
    IsZero,

    // Collection/Array Operators
    ContainsAll,
    ContainsAny,
    ContainsNone,
    IsEmpty,
    IsNotEmpty,
    HasUniqueItems,

    // String Operators
    IsAlphabetic,
    IsAlphanumeric,
    IsLowerCase,
    IsUpperCase,
    ContainsWord,

    // Comparison with Tolerance
    ApproximatelyEquals,
}

impl Display for ComparisonOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for ComparisonOperator {
    type Err = TypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Equals" => Ok(ComparisonOperator::Equals),
            "NotEqual" => Ok(ComparisonOperator::NotEqual),
            "GreaterThan" => Ok(ComparisonOperator::GreaterThan),
            "GreaterThanOrEqual" => Ok(ComparisonOperator::GreaterThanOrEqual),
            "LessThan" => Ok(ComparisonOperator::LessThan),
            "LessThanOrEqual" => Ok(ComparisonOperator::LessThanOrEqual),
            "Contains" => Ok(ComparisonOperator::Contains),
            "NotContains" => Ok(ComparisonOperator::NotContains),
            "StartsWith" => Ok(ComparisonOperator::StartsWith),
            "EndsWith" => Ok(ComparisonOperator::EndsWith),
            "Matches" => Ok(ComparisonOperator::Matches),
            "HasLengthEqual" => Ok(ComparisonOperator::HasLengthEqual),
            "HasLengthGreaterThan" => Ok(ComparisonOperator::HasLengthGreaterThan),
            "HasLengthLessThan" => Ok(ComparisonOperator::HasLengthLessThan),
            "HasLengthGreaterThanOrEqual" => Ok(ComparisonOperator::HasLengthGreaterThanOrEqual),
            "HasLengthLessThanOrEqual" => Ok(ComparisonOperator::HasLengthLessThanOrEqual),

            // Type Validation
            "IsNumeric" => Ok(ComparisonOperator::IsNumeric),
            "IsString" => Ok(ComparisonOperator::IsString),
            "IsBoolean" => Ok(ComparisonOperator::IsBoolean),
            "IsNull" => Ok(ComparisonOperator::IsNull),
            "IsArray" => Ok(ComparisonOperator::IsArray),
            "IsObject" => Ok(ComparisonOperator::IsObject),

            // Pattern & Format
            "IsEmail" => Ok(ComparisonOperator::IsEmail),
            "IsUrl" => Ok(ComparisonOperator::IsUrl),
            "IsUuid" => Ok(ComparisonOperator::IsUuid),
            "IsIso8601" => Ok(ComparisonOperator::IsIso8601),
            "IsJson" => Ok(ComparisonOperator::IsJson),
            "MatchesRegex" => Ok(ComparisonOperator::MatchesRegex),

            // Numeric Range
            "InRange" => Ok(ComparisonOperator::InRange),
            "NotInRange" => Ok(ComparisonOperator::NotInRange),
            "IsPositive" => Ok(ComparisonOperator::IsPositive),
            "IsNegative" => Ok(ComparisonOperator::IsNegative),
            "IsZero" => Ok(ComparisonOperator::IsZero),

            // Collection/Array
            "ContainsAll" => Ok(ComparisonOperator::ContainsAll),
            "ContainsAny" => Ok(ComparisonOperator::ContainsAny),
            "ContainsNone" => Ok(ComparisonOperator::ContainsNone),
            "IsEmpty" => Ok(ComparisonOperator::IsEmpty),
            "IsNotEmpty" => Ok(ComparisonOperator::IsNotEmpty),
            "HasUniqueItems" => Ok(ComparisonOperator::HasUniqueItems),

            // String
            "IsAlphabetic" => Ok(ComparisonOperator::IsAlphabetic),
            "IsAlphanumeric" => Ok(ComparisonOperator::IsAlphanumeric),
            "IsLowerCase" => Ok(ComparisonOperator::IsLowerCase),
            "IsUpperCase" => Ok(ComparisonOperator::IsUpperCase),
            "ContainsWord" => Ok(ComparisonOperator::ContainsWord),

            // Tolerance
            "ApproximatelyEquals" => Ok(ComparisonOperator::ApproximatelyEquals),

            _ => Err(TypeError::InvalidCompressionTypeError),
        }
    }
}

impl ComparisonOperator {
    pub fn as_str(&self) -> &str {
        match self {
            ComparisonOperator::Equals => "Equals",
            ComparisonOperator::NotEqual => "NotEqual",
            ComparisonOperator::GreaterThan => "GreaterThan",
            ComparisonOperator::GreaterThanOrEqual => "GreaterThanOrEqual",
            ComparisonOperator::LessThan => "LessThan",
            ComparisonOperator::LessThanOrEqual => "LessThanOrEqual",
            ComparisonOperator::Contains => "Contains",
            ComparisonOperator::NotContains => "NotContains",
            ComparisonOperator::StartsWith => "StartsWith",
            ComparisonOperator::EndsWith => "EndsWith",
            ComparisonOperator::Matches => "Matches",
            ComparisonOperator::HasLengthEqual => "HasLengthEqual",
            ComparisonOperator::HasLengthGreaterThan => "HasLengthGreaterThan",
            ComparisonOperator::HasLengthLessThan => "HasLengthLessThan",
            ComparisonOperator::HasLengthGreaterThanOrEqual => "HasLengthGreaterThanOrEqual",
            ComparisonOperator::HasLengthLessThanOrEqual => "HasLengthLessThanOrEqual",

            // Type Validation
            ComparisonOperator::IsNumeric => "IsNumeric",
            ComparisonOperator::IsString => "IsString",
            ComparisonOperator::IsBoolean => "IsBoolean",
            ComparisonOperator::IsNull => "IsNull",
            ComparisonOperator::IsArray => "IsArray",
            ComparisonOperator::IsObject => "IsObject",

            // Pattern & Format
            ComparisonOperator::IsEmail => "IsEmail",
            ComparisonOperator::IsUrl => "IsUrl",
            ComparisonOperator::IsUuid => "IsUuid",
            ComparisonOperator::IsIso8601 => "IsIso8601",
            ComparisonOperator::IsJson => "IsJson",
            ComparisonOperator::MatchesRegex => "MatchesRegex",

            // Numeric Range
            ComparisonOperator::InRange => "InRange",
            ComparisonOperator::NotInRange => "NotInRange",
            ComparisonOperator::IsPositive => "IsPositive",
            ComparisonOperator::IsNegative => "IsNegative",
            ComparisonOperator::IsZero => "IsZero",

            // Collection/Array
            ComparisonOperator::ContainsAll => "ContainsAll",
            ComparisonOperator::ContainsAny => "ContainsAny",
            ComparisonOperator::ContainsNone => "ContainsNone",
            ComparisonOperator::IsEmpty => "IsEmpty",
            ComparisonOperator::IsNotEmpty => "IsNotEmpty",
            ComparisonOperator::HasUniqueItems => "HasUniqueItems",

            // String
            ComparisonOperator::IsAlphabetic => "IsAlphabetic",
            ComparisonOperator::IsAlphanumeric => "IsAlphanumeric",
            ComparisonOperator::IsLowerCase => "IsLowerCase",
            ComparisonOperator::IsUpperCase => "IsUpperCase",
            ComparisonOperator::ContainsWord => "ContainsWord",

            // Tolerance
            ComparisonOperator::ApproximatelyEquals => "ApproximatelyEquals",
        }
    }
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
            ComparisonOperator::HasLengthEqual
            | ComparisonOperator::HasLengthGreaterThan
            | ComparisonOperator::HasLengthLessThan
            | ComparisonOperator::HasLengthGreaterThanOrEqual
            | ComparisonOperator::HasLengthLessThanOrEqual => match self {
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

impl Display for EvaluationTaskType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let task_type_str = match self {
            EvaluationTaskType::Assertion => "Assertion",
            EvaluationTaskType::LLMJudge => "LLMJudge",
            EvaluationTaskType::HumanValidation => "HumanValidation",
        };
        write!(f, "{}", task_type_str)
    }
}

impl FromStr for EvaluationTaskType {
    type Err = TypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Assertion" => Ok(EvaluationTaskType::Assertion),
            "LLMJudge" => Ok(EvaluationTaskType::LLMJudge),
            "HumanValidation" => Ok(EvaluationTaskType::HumanValidation),
            _ => Err(TypeError::InvalidEvalType(s.to_string())),
        }
    }
}

impl EvaluationTaskType {
    pub fn as_str(&self) -> &str {
        match self {
            EvaluationTaskType::Assertion => "Assertion",
            EvaluationTaskType::LLMJudge => "LLMJudge",
            EvaluationTaskType::HumanValidation => "HumanValidation",
        }
    }
}
