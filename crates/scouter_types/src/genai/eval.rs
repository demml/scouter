use crate::error::TypeError;
use crate::genai::traits::TaskAccessor;
use crate::PyHelperFuncs;
use core::fmt::Debug;
use potato_head::prompt_types::Prompt;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyFloat, PyInt, PyList, PyString};
use pythonize::{depythonize, pythonize};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Display;
use std::str::FromStr;

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssertionResult {
    #[pyo3(get)]
    pub passed: bool,
    pub actual: Value,

    #[pyo3(get)]
    pub message: String,

    pub expected: Value,
}

impl AssertionResult {
    pub fn new(passed: bool, actual: Value, message: String, expected: Value) -> Self {
        Self {
            passed,
            actual,
            message,
            expected,
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

#[pymethods]
impl AssertionResult {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }

    #[getter]
    pub fn get_actual<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyAny>, TypeError> {
        let py_value = pythonize(py, &self.actual)?;
        Ok(py_value)
    }

    #[getter]
    pub fn get_expected<'py>(&self, py: Python<'py>) -> Result<Bound<'py, PyAny>, TypeError> {
        let py_value = pythonize(py, &self.expected)?;
        Ok(py_value)
    }
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssertionResults {
    #[pyo3(get)]
    pub results: HashMap<String, AssertionResult>,
}

#[pymethods]
impl AssertionResults {
    pub fn __str__(&self) -> String {
        PyHelperFuncs::__str__(self)
    }

    pub fn __getitem__(&self, key: &str) -> Result<AssertionResult, TypeError> {
        if let Some(result) = self.results.get(key) {
            Ok(result.clone())
        } else {
            Err(TypeError::KeyNotFound {
                key: key.to_string(),
            })
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

    pub condition: bool,
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
    #[pyo3(signature = (id, field_path, expected_value, operator, description=None, depends_on=None, condition=None))]
    pub fn new(
        id: String,
        field_path: Option<String>,
        expected_value: &Bound<'_, PyAny>,
        operator: ComparisonOperator,
        description: Option<String>,
        depends_on: Option<Vec<String>>,
        condition: Option<bool>,
    ) -> Result<Self, TypeError> {
        let expected_value = depythonize(expected_value)?;
        let condition = condition.unwrap_or(false);

        Ok(Self {
            id: id.to_lowercase(),
            field_path,
            operator,
            expected_value,
            description,
            task_type: EvaluationTaskType::Assertion,
            depends_on: depends_on.unwrap_or_default(),
            result: None,
            condition,
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

    pub description: Option<String>,

    #[pyo3(get, set)]
    pub condition: bool,
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
    #[pyo3(signature = (id, prompt, expected_value,  field_path,operator, description=None, depends_on=None, max_retries=None, condition=None))]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: &str,
        prompt: Prompt,
        expected_value: &Bound<'_, PyAny>,
        field_path: Option<String>,
        operator: ComparisonOperator,
        description: Option<String>,
        depends_on: Option<Vec<String>>,
        max_retries: Option<u32>,
        condition: Option<bool>,
    ) -> Result<Self, TypeError> {
        let expected_value = depythonize(expected_value)?;

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
            description,
            condition: condition.unwrap_or(false),
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
    #[allow(clippy::too_many_arguments)]
    pub fn new_rs(
        id: &str,
        prompt: Prompt,
        expected_value: Value,
        field_path: Option<String>,
        operator: ComparisonOperator,
        depends_on: Option<Vec<String>>,
        max_retries: Option<u32>,
        description: Option<String>,
        condition: Option<bool>,
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
            description,
            condition: condition.unwrap_or(false),
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

#[pyclass(eq, eq_int)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SpanStatus {
    Ok,
    Error,
    Unset,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PyValueWrapper(pub Value);

impl<'py> IntoPyObject<'py> for PyValueWrapper {
    type Target = PyAny; // the Python type
    type Output = Bound<'py, Self::Target>;
    type Error = std::convert::Infallible;

    fn into_pyobject(self, py: Python<'py>) -> Result<Self::Output, Self::Error> {
        Ok(pythonize(py, &self.0).unwrap())
    }
}

impl<'a, 'py> FromPyObject<'a, 'py> for PyValueWrapper {
    type Error = TypeError;

    fn extract(ob: pyo3::Borrowed<'a, 'py, pyo3::PyAny>) -> Result<Self, Self::Error> {
        let value: Value = depythonize(&ob)?;
        Ok(PyValueWrapper(value))
    }
}

/// Filter configuration for selecting spans to assert on
#[pyclass(eq)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SpanFilter {
    /// Match spans by exact name
    ByName { name: String },

    /// Match spans by name pattern (regex)
    ByNamePattern { pattern: String },

    /// Match spans with specific attribute key
    WithAttribute { key: String },

    /// Match spans with specific attribute key-value pair
    WithAttributeValue { key: String, value: PyValueWrapper },

    /// Match spans by status code
    WithStatus { status: SpanStatus },

    /// Match spans with duration constraints
    WithDuration {
        min_ms: Option<f64>,
        max_ms: Option<f64>,
    },

    /// Match a sequence of span names in order
    Sequence { names: Vec<String> },

    /// Combine multiple filters with AND logic
    And { filters: Vec<SpanFilter> },

    /// Combine multiple filters with OR logic
    Or { filters: Vec<SpanFilter> },
}

#[pymethods]
impl SpanFilter {
    #[staticmethod]
    pub fn by_name(name: String) -> Self {
        SpanFilter::ByName { name }
    }

    #[staticmethod]
    pub fn by_name_pattern(pattern: String) -> Self {
        SpanFilter::ByNamePattern { pattern }
    }

    #[staticmethod]
    pub fn with_attribute(key: String) -> Self {
        SpanFilter::WithAttribute { key }
    }

    #[staticmethod]
    pub fn with_attribute_value(key: String, value: &Bound<'_, PyAny>) -> Result<Self, TypeError> {
        let value = PyValueWrapper(depythonize(value).unwrap());
        Ok(SpanFilter::WithAttributeValue { key, value })
    }

    #[staticmethod]
    pub fn with_status(status: SpanStatus) -> Self {
        SpanFilter::WithStatus { status }
    }

    #[staticmethod]
    #[pyo3(signature = (min_ms=None, max_ms=None))]
    pub fn with_duration(min_ms: Option<f64>, max_ms: Option<f64>) -> Self {
        SpanFilter::WithDuration { min_ms, max_ms }
    }

    #[staticmethod]
    pub fn sequence(names: Vec<String>) -> Self {
        SpanFilter::Sequence { names }
    }

    pub fn and_(&self, other: SpanFilter) -> Self {
        match self {
            SpanFilter::And { filters } => {
                let mut new_filters = filters.clone();
                new_filters.push(other);
                SpanFilter::And {
                    filters: new_filters,
                }
            }
            _ => SpanFilter::And {
                filters: vec![self.clone(), other],
            },
        }
    }

    pub fn or_(&self, other: SpanFilter) -> Self {
        match self {
            SpanFilter::Or { filters } => {
                let mut new_filters = filters.clone();
                new_filters.push(other);
                SpanFilter::Or {
                    filters: new_filters,
                }
            }
            _ => SpanFilter::Or {
                filters: vec![self.clone(), other],
            },
        }
    }
}

#[pyclass(eq, eq_int)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AggregationType {
    Count,
    Sum,
    Average,
    Min,
    Max,
    First,
    Last,
}

/// Unified assertion target that can operate on traces or filtered spans
#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TraceAssertion {
    /// Check if spans exist in a specific order
    SpanSequence { span_names: Vec<String> },

    /// Check if all specified span names exist (order doesn't matter)
    SpanSet { span_names: Vec<String> },

    /// Count spans matching a filter
    SpanCount { filter: SpanFilter },

    /// Check if any span matching filter exists
    SpanExists { filter: SpanFilter },

    /// Get attribute value from span(s) matching filter
    SpanAttribute {
        filter: SpanFilter,
        attribute_key: String,
    },

    /// Get duration of span(s) matching filter
    SpanDuration { filter: SpanFilter },

    /// Aggregate a numeric attribute across filtered spans
    SpanAggregation {
        filter: SpanFilter,
        attribute_key: String,
        aggregation: AggregationType,
    },

    /// Check total duration of entire trace
    TraceDuration {},

    /// Count total spans in trace
    TraceSpanCount {},

    /// Count spans with errors in trace
    TraceErrorCount {},

    /// Count unique services in trace
    TraceServiceCount {},

    /// Get maximum depth of span tree
    TraceMaxDepth {},

    /// Get trace-level attribute
    TraceAttribute { attribute_key: String },
}

// implement to_string for TraceAssertion
impl Display for TraceAssertion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // serde serialize to string
        let s = serde_json::to_string(self).unwrap_or_default();
        write!(f, "{}", s)
    }
}

#[pymethods]
impl TraceAssertion {
    #[staticmethod]
    pub fn span_sequence(span_names: Vec<String>) -> Self {
        TraceAssertion::SpanSequence { span_names }
    }

    #[staticmethod]
    pub fn span_set(span_names: Vec<String>) -> Self {
        TraceAssertion::SpanSet { span_names }
    }

    #[staticmethod]
    pub fn span_count(filter: SpanFilter) -> Self {
        TraceAssertion::SpanCount { filter }
    }

    #[staticmethod]
    pub fn span_exists(filter: SpanFilter) -> Self {
        TraceAssertion::SpanExists { filter }
    }

    #[staticmethod]
    pub fn span_attribute(filter: SpanFilter, attribute_key: String) -> Self {
        TraceAssertion::SpanAttribute {
            filter,
            attribute_key,
        }
    }

    #[staticmethod]
    pub fn span_duration(filter: SpanFilter) -> Self {
        TraceAssertion::SpanDuration { filter }
    }

    #[staticmethod]
    pub fn span_aggregation(
        filter: SpanFilter,
        attribute_key: String,
        aggregation: AggregationType,
    ) -> Self {
        TraceAssertion::SpanAggregation {
            filter,
            attribute_key,
            aggregation,
        }
    }

    #[staticmethod]
    pub fn trace_duration() -> Self {
        TraceAssertion::TraceDuration {}
    }

    #[staticmethod]
    pub fn trace_span_count() -> Self {
        TraceAssertion::TraceSpanCount {}
    }

    #[staticmethod]
    pub fn trace_error_count() -> Self {
        TraceAssertion::TraceErrorCount {}
    }

    #[staticmethod]
    pub fn trace_service_count() -> Self {
        TraceAssertion::TraceServiceCount {}
    }

    #[staticmethod]
    pub fn trace_max_depth() -> Self {
        TraceAssertion::TraceMaxDepth {}
    }

    #[staticmethod]
    pub fn trace_attribute(attribute_key: String) -> Self {
        TraceAssertion::TraceAttribute { attribute_key }
    }

    pub fn model_dump_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TraceAssertionTask {
    #[pyo3(get, set)]
    pub id: String,

    #[pyo3(get, set)]
    pub assertion: TraceAssertion,

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

    #[pyo3(get, set)]
    pub condition: bool,
}

#[pymethods]
impl TraceAssertionTask {
    /// Creates a new TraceAssertionTask
    ///
    /// # Examples
    /// ```python
    /// # Check execution order of spans
    /// task = TraceAssertionTask(
    ///     id="verify_agent_workflow",
    ///     assertion=TraceAssertion.span_sequence(["call_tool", "run_agent", "double_check"]),
    ///     operator=ComparisonOperator.SequenceMatches,
    ///     expected_value=True
    /// )
    ///
    /// # Check all required spans exist
    /// task = TraceAssertionTask(
    ///     id="verify_required_steps",
    ///     assertion=TraceAssertion.span_set(["call_tool", "run_agent", "double_check"]),
    ///     operator=ComparisonOperator.ContainsAll,
    ///     expected_value=True
    /// )
    ///
    /// # Check total trace duration
    /// task = TraceAssertionTask(
    ///     id="verify_performance",
    ///     assertion=TraceAssertion.trace_duration(),
    ///     operator=ComparisonOperator.LessThan,
    ///     expected_value=5000.0
    /// )
    ///
    /// # Check count of specific spans
    /// task = TraceAssertionTask(
    ///     id="verify_retry_count",
    ///     assertion=TraceAssertion.span_count(
    ///         SpanFilter.by_name("retry_operation")
    ///     ),
    ///     operator=ComparisonOperator.LessThanOrEqual,
    ///     expected_value=3
    /// )
    ///
    /// # Check span attribute
    /// task = TraceAssertionTask(
    ///     id="verify_model_used",
    ///     assertion=TraceAssertion.span_attribute(
    ///         SpanFilter.by_name("llm.generate"),
    ///         "model"
    ///     ),
    ///     operator=ComparisonOperator.Equals,
    ///     expected_value="gpt-4"
    /// )
    /// ```
    #[new]
    /// Creates a new TraceAssertionTask
    #[pyo3(signature = (id, assertion, expected_value, operator, description=None, depends_on=None, condition=None))]
    pub fn new(
        id: String,
        assertion: TraceAssertion,
        expected_value: &Bound<'_, PyAny>,
        operator: ComparisonOperator,
        description: Option<String>,
        depends_on: Option<Vec<String>>,
        condition: Option<bool>,
    ) -> Result<Self, TypeError> {
        let expected_value = depythonize(expected_value)?;

        Ok(Self {
            id: id.to_lowercase(),
            assertion,
            operator,
            expected_value,
            description,
            task_type: EvaluationTaskType::TraceAssertion,
            depends_on: depends_on.unwrap_or_default(),
            result: None,
            condition: condition.unwrap_or(false),
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

impl TaskAccessor for TraceAssertionTask {
    fn field_path(&self) -> Option<&str> {
        None
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

#[derive(Debug, Clone)]
pub enum EvaluationTask {
    Assertion(Box<AssertionTask>),
    LLMJudge(Box<LLMJudgeTask>),
    TraceAssertion(Box<TraceAssertionTask>),
}

impl TaskAccessor for EvaluationTask {
    fn field_path(&self) -> Option<&str> {
        match self {
            EvaluationTask::Assertion(t) => t.field_path(),
            EvaluationTask::LLMJudge(t) => t.field_path(),
            EvaluationTask::TraceAssertion(t) => t.field_path(),
        }
    }

    fn id(&self) -> &str {
        match self {
            EvaluationTask::Assertion(t) => t.id(),
            EvaluationTask::LLMJudge(t) => t.id(),
            EvaluationTask::TraceAssertion(t) => t.id(),
        }
    }

    fn task_type(&self) -> &EvaluationTaskType {
        match self {
            EvaluationTask::Assertion(t) => t.task_type(),
            EvaluationTask::LLMJudge(t) => t.task_type(),
            EvaluationTask::TraceAssertion(t) => t.task_type(),
        }
    }

    fn operator(&self) -> &ComparisonOperator {
        match self {
            EvaluationTask::Assertion(t) => t.operator(),
            EvaluationTask::LLMJudge(t) => t.operator(),
            EvaluationTask::TraceAssertion(t) => t.operator(),
        }
    }

    fn expected_value(&self) -> &Value {
        match self {
            EvaluationTask::Assertion(t) => t.expected_value(),
            EvaluationTask::LLMJudge(t) => t.expected_value(),
            EvaluationTask::TraceAssertion(t) => t.expected_value(),
        }
    }

    fn depends_on(&self) -> &[String] {
        match self {
            EvaluationTask::Assertion(t) => t.depends_on(),
            EvaluationTask::LLMJudge(t) => t.depends_on(),
            EvaluationTask::TraceAssertion(t) => t.depends_on(),
        }
    }

    fn add_result(&mut self, result: AssertionResult) {
        match self {
            EvaluationTask::Assertion(t) => t.add_result(result),
            EvaluationTask::LLMJudge(t) => t.add_result(result),
            EvaluationTask::TraceAssertion(t) => t.add_result(result),
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

impl From<AssertionTask> for EvaluationTask {
    fn from(task: AssertionTask) -> Self {
        EvaluationTask::Assertion(Box::new(task))
    }
}

impl From<LLMJudgeTask> for EvaluationTask {
    fn from(task: LLMJudgeTask) -> Self {
        EvaluationTask::LLMJudge(Box::new(task))
    }
}

impl From<TraceAssertionTask> for EvaluationTask {
    fn from(task: TraceAssertionTask) -> Self {
        EvaluationTask::TraceAssertion(Box::new(task))
    }
}

impl Default for EvaluationTasks {
    fn default() -> Self {
        Self::new()
    }
}

#[pyclass(eq, eq_int)]
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
    SequenceMatches,
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
            "SequenceMatches" => Ok(ComparisonOperator::SequenceMatches),

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
            ComparisonOperator::SequenceMatches => "SequenceMatches",

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
    Conditional,
    HumanValidation,
    TraceAssertion,
}

impl Display for EvaluationTaskType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let task_type_str = match self {
            EvaluationTaskType::Assertion => "Assertion",
            EvaluationTaskType::LLMJudge => "LLMJudge",
            EvaluationTaskType::Conditional => "Conditional",
            EvaluationTaskType::HumanValidation => "HumanValidation",
            EvaluationTaskType::TraceAssertion => "TraceAssertion",
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
            "Conditional" => Ok(EvaluationTaskType::Conditional),
            "HumanValidation" => Ok(EvaluationTaskType::HumanValidation),
            "TraceAssertion" => Ok(EvaluationTaskType::TraceAssertion),
            _ => Err(TypeError::InvalidEvalType(s.to_string())),
        }
    }
}

impl EvaluationTaskType {
    pub fn as_str(&self) -> &str {
        match self {
            EvaluationTaskType::Assertion => "Assertion",
            EvaluationTaskType::LLMJudge => "LLMJudge",
            EvaluationTaskType::Conditional => "Conditional",
            EvaluationTaskType::HumanValidation => "HumanValidation",
            EvaluationTaskType::TraceAssertion => "TraceAssertion",
        }
    }
}
