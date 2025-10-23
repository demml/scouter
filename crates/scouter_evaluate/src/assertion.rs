use crate::error::EvaluationError;
use pyo3::prelude::*;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::OnceLock;

const REGEX_FIELD_PARSE_PATTERN: &str = r"[a-zA-Z_][a-zA-Z0-9_]*|\[[0-9]+\]";

pub struct FieldEvaluator;

/// Utility for extracting field values from JSON-like structures
/// Supports: "field", "field.subfield", "field[0]", "field[0].subfield"
impl FieldEvaluator {
    pub fn extract_field_value(json: &Value, field_path: &str) -> Result<Value, EvaluationError> {
        let path_segments = Self::parse_field_path(field_path)?;
        let mut current_value = json;

        for segment in path_segments {
            current_value = match segment {
                PathSegment::Field(field_name) => current_value
                    .get(&field_name)
                    .ok_or_else(|| EvaluationError::FieldNotFound(field_name))?,
                PathSegment::Index(index) => current_value
                    .get(index)
                    .ok_or_else(|| EvaluationError::IndexNotFound(index))?,
            };
        }

        Ok(current_value.clone())
    }

    fn parse_field_path(path: &str) -> Result<Vec<PathSegment>, EvaluationError> {
        static PATH_REGEX: OnceLock<Regex> = OnceLock::new();
        let regex = PATH_REGEX.get_or_init(|| {
            Regex::new(REGEX_FIELD_PARSE_PATTERN)
                .expect("Invalid regex pattern in REGEX_FIELD_PARSE_PATTERN")
        });

        let mut segments = Vec::new();

        for capture in regex.find_iter(path) {
            let segment_str = capture.as_str();

            if segment_str.starts_with('[') && segment_str.ends_with(']') {
                // Array index: [0], [1], etc.
                let index_str = &segment_str[1..segment_str.len() - 1];
                let index: usize = index_str
                    .parse()
                    .map_err(|_| EvaluationError::InvalidArrayIndex(index_str.to_string()))?;
                segments.push(PathSegment::Index(index));
            } else {
                // Field name: field, subfield, etc.
                segments.push(PathSegment::Field(segment_str.to_string()));
            }
        }

        if segments.is_empty() {
            return Err(EvaluationError::EmptyFieldPath);
        }

        Ok(segments)
    }
}

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
fn assertion_value_from_py(value: &Bound<'_, PyAny>) -> Result<AssertionValue, EvaluationError> {
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
        Err(EvaluationError::InvalidAssertionValueType)
    }
}

#[pyclass]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldAssertion {
    #[pyo3(get, set)]
    pub field_path: String,

    #[pyo3(get, set)]
    pub operator: ComparisonOperator,

    #[pyo3(get, set)]
    pub expected_value: AssertionValue,

    pub description: Option<String>,
}

#[pymethods]
impl FieldAssertion {
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
pub struct AssertionSuite {
    #[pyo3(get, set)]
    pub name: String,

    #[pyo3(get, set)]
    pub assertions: Vec<FieldAssertion>,
}

#[pyclass]
#[derive(Debug, Clone)]
pub struct AssertionEvaluator;

#[pymethods]
impl AssertionEvaluator {
    #[new]
    pub fn new() -> Self {
        Self
    }

    /// Evaluate a suite of assertions against the provided JSON output
    /// # Arguments
    /// * `json_output`: The JSON string output from the LLM
    /// * `assertion_suite`: The suite of assertions to evaluate
    /// Returns a list of AssertionResult objects
    #[pyo3(signature = (json_output, assertion_suite))]
    pub fn evaluate_suite(
        &self,
        json_output: &str,
        assertion_suite: &AssertionSuite,
    ) -> Result<Vec<AssertionResult>, EvaluationError> {
        let json_value: Value = serde_json::from_str(json_output)?;

        let results = assertion_suite
            .assertions
            .iter()
            .map(|assertion| self.evaluate_assertion(&json_value, assertion))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(results)
    }

    #[pyo3(signature = (json_output, assertion))]
    pub fn evaluate_single(
        &self,
        json_output: &str,
        assertion: &FieldAssertion,
    ) -> Result<AssertionResult, EvaluationError> {
        let json_value: Value = serde_json::from_str(json_output)?;
        self.evaluate_assertion(&json_value, assertion)
    }
}

impl AssertionEvaluator {
    /// Evaluates a single assertion against the provided JSON value
    /// # Arguments
    /// * `json_value`: The JSON value to evaluate against
    /// * `assertion`: The FieldAssertion to evaluate
    /// Returns an AssertionResult indicating pass/fail and details
    /// # Errors
    /// Returns EvaluationError if field extraction or comparison fails
    pub fn evaluate_assertion(
        &self,
        json_value: &Value,
        assertion: &FieldAssertion,
    ) -> Result<AssertionResult, EvaluationError> {
        // Extract the actual value from the JSON
        let actual_value = FieldEvaluator::extract_field_value(json_value, &assertion.field_path)?;

        // Convert to comparable format
        let actual_comparable =
            self.value_to_assertion_value(&actual_value, &assertion.operator)?;

        // Perform the comparison
        let passed = self.compare_values(
            &actual_comparable,
            &assertion.operator,
            &assertion.expected_value,
        )?;

        // Create result
        Ok(AssertionResult {
            passed,
            field_path: assertion.field_path.clone(),
            expected: format!("{:?}", assertion.expected_value),
            actual: format!("{:?}", actual_comparable),
            message: if passed {
                format!("✓ Field '{}' assertion passed", assertion.field_path)
            } else {
                format!(
                    "✗ Field '{}' assertion failed: expected {:?}, got {:?}",
                    assertion.field_path, assertion.expected_value, actual_comparable
                )
            },
        })
    }

    fn compare_values(
        &self,
        actual: &AssertionValue,
        operator: &ComparisonOperator,
        expected: &AssertionValue,
    ) -> Result<bool, EvaluationError> {
        match operator {
            ComparisonOperator::HasLength => self.check_length(actual, expected),
            ComparisonOperator::Equal => Ok(actual == expected),
            ComparisonOperator::NotEqual => Ok(actual != expected),
            ComparisonOperator::GreaterThan => self.compare_numeric(actual, expected, |a, b| a > b),
            ComparisonOperator::GreaterThanOrEqual => {
                self.compare_numeric(actual, expected, |a, b| a >= b)
            }
            ComparisonOperator::LessThan => self.compare_numeric(actual, expected, |a, b| a < b),
            ComparisonOperator::LessThanOrEqual => {
                self.compare_numeric(actual, expected, |a, b| a <= b)
            }
            ComparisonOperator::Contains => self.check_contains(actual, expected),
            ComparisonOperator::NotContains => Ok(!self.check_contains(actual, expected)?),
            ComparisonOperator::StartsWith => self.check_starts_with(actual, expected),
            ComparisonOperator::EndsWith => self.check_ends_with(actual, expected),
            ComparisonOperator::Matches => self.check_regex_match(actual, expected),
        }
    }

    fn check_length(
        &self,
        actual_value: &AssertionValue,
        expected_value: &AssertionValue,
    ) -> Result<bool, EvaluationError> {
        let actual_length = match actual_value {
            AssertionValue::List(list) => list.len(),
            AssertionValue::String(s) => s.len(),
            _ => return Err(EvaluationError::CannotGetLengthOfObject),
        };

        match expected_value {
            AssertionValue::Integer(expected_len) => Ok(actual_length == *expected_len as usize),
            _ => Err(EvaluationError::ExpectedLengthMustBeInteger),
        }
    }

    fn compare_numeric<F>(
        &self,
        actual: &AssertionValue,
        expected: &AssertionValue,
        comparator: F,
    ) -> Result<bool, EvaluationError>
    where
        F: Fn(f64, f64) -> bool,
    {
        let actual_num = self.to_numeric(actual)?;
        let expected_num = self.to_numeric(expected)?;
        Ok(comparator(actual_num, expected_num))
    }

    fn to_numeric(&self, value: &AssertionValue) -> Result<f64, EvaluationError> {
        match value {
            AssertionValue::Number(n) => Ok(*n),
            AssertionValue::Integer(i) => Ok(*i as f64),
            _ => Err(EvaluationError::CannotCompareNonNumericValues),
        }
    }

    fn check_contains(
        &self,
        actual: &AssertionValue,
        expected: &AssertionValue,
    ) -> Result<bool, EvaluationError> {
        match (actual, expected) {
            (AssertionValue::String(s), AssertionValue::String(substr)) => Ok(s.contains(substr)),
            (AssertionValue::List(list), expected_item) => Ok(list.contains(expected_item)),
            _ => Err(EvaluationError::InvalidContainsOperation),
        }
    }

    fn check_starts_with(
        &self,
        actual: &AssertionValue,
        expected: &AssertionValue,
    ) -> Result<bool, EvaluationError> {
        match (actual, expected) {
            (AssertionValue::String(s), AssertionValue::String(prefix)) => {
                Ok(s.starts_with(prefix))
            }
            _ => Err(EvaluationError::InvalidStartsWithOperation),
        }
    }

    fn check_ends_with(
        &self,
        actual: &AssertionValue,
        expected: &AssertionValue,
    ) -> Result<bool, EvaluationError> {
        match (actual, expected) {
            (AssertionValue::String(s), AssertionValue::String(suffix)) => Ok(s.ends_with(suffix)),
            _ => Err(EvaluationError::InvalidEndsWithOperation),
        }
    }

    fn check_regex_match(
        &self,
        actual: &AssertionValue,
        expected: &AssertionValue,
    ) -> Result<bool, EvaluationError> {
        match (actual, expected) {
            (AssertionValue::String(s), AssertionValue::String(pattern)) => {
                let regex = Regex::new(pattern)?;
                Ok(regex.is_match(s))
            }
            _ => Err(EvaluationError::InvalidRegexOperation),
        }
    }

    fn value_to_assertion_value(
        &self,
        value: &Value,
        comparator: &ComparisonOperator,
    ) -> Result<AssertionValue, EvaluationError> {
        match value {
            Value::String(s) => Ok(AssertionValue::String(s.clone())),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(AssertionValue::Integer(i))
                } else if let Some(f) = n.as_f64() {
                    Ok(AssertionValue::Number(f))
                } else {
                    Err(EvaluationError::InvalidNumberFormat)
                }
            }
            Value::Bool(b) => Ok(AssertionValue::Boolean(*b)),
            Value::Array(arr) => match comparator {
                // need to account for comparisons that checks length
                ComparisonOperator::HasLength
                | ComparisonOperator::LessThan
                | ComparisonOperator::LessThanOrEqual
                | ComparisonOperator::GreaterThan
                | ComparisonOperator::GreaterThanOrEqual => {
                    Ok(AssertionValue::Integer(arr.len() as i64))
                }
                _ => {
                    let converted: Result<Vec<_>, _> = arr
                        .iter()
                        .map(|v| self.value_to_assertion_value(v, comparator))
                        .collect();
                    Ok(AssertionValue::List(converted?))
                }
            },
            Value::Null => Ok(AssertionValue::Null()),
            Value::Object(_) => Err(EvaluationError::CannotConvertObjectToAssertionValue),
        }
    }
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
    /// Returns a FieldAssertion object
    /// # Errors
    /// Returns EvaluationError if the expected value cannot be converted
    pub fn assert(
        &self,
        comparison: ComparisonOperator,
        value: &Bound<'_, PyAny>,
        description: Option<String>,
    ) -> Result<FieldAssertion, EvaluationError> {
        Ok(FieldAssertion {
            field_path: self.field_path.clone(),
            operator: comparison,
            expected_value: assertion_value_from_py(value)?,
            description,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Test data matching your StructuredTaskOutput example
    fn get_test_json() -> Value {
        json!({
            "tasks": ["task1", "task2", "task3"],
            "status": "in_progress",
            "metadata": {
                "created_by": "user_123",
                "priority": "high",
                "tags": ["urgent", "backend"],
                "nested": {
                    "deep": {
                        "value": "found_it"
                    }
                }
            },
            "counts": {
                "total": 42,
                "completed": 15
            },
            "empty_array": [],
            "single_item": ["only_one"]
        })
    }

    #[test]
    fn test_parse_field_path_simple_field() {
        let segments = FieldEvaluator::parse_field_path("status").unwrap();
        assert_eq!(segments, vec![PathSegment::Field("status".to_string())]);
    }

    #[test]
    fn test_parse_field_path_nested_field() {
        let segments = FieldEvaluator::parse_field_path("metadata.created_by").unwrap();
        assert_eq!(
            segments,
            vec![
                PathSegment::Field("metadata".to_string()),
                PathSegment::Field("created_by".to_string())
            ]
        );
    }

    #[test]
    fn test_parse_field_path_array_index() {
        let segments = FieldEvaluator::parse_field_path("tasks[0]").unwrap();
        assert_eq!(
            segments,
            vec![
                PathSegment::Field("tasks".to_string()),
                PathSegment::Index(0)
            ]
        );
    }

    #[test]
    fn test_parse_field_path_complex() {
        let segments = FieldEvaluator::parse_field_path("metadata.tags[1]").unwrap();
        assert_eq!(
            segments,
            vec![
                PathSegment::Field("metadata".to_string()),
                PathSegment::Field("tags".to_string()),
                PathSegment::Index(1)
            ]
        );
    }

    #[test]
    fn test_parse_field_path_deep_nested() {
        let segments = FieldEvaluator::parse_field_path("metadata.nested.deep.value").unwrap();
        assert_eq!(
            segments,
            vec![
                PathSegment::Field("metadata".to_string()),
                PathSegment::Field("nested".to_string()),
                PathSegment::Field("deep".to_string()),
                PathSegment::Field("value".to_string())
            ]
        );
    }

    #[test]
    fn test_parse_field_path_underscore_field() {
        let segments = FieldEvaluator::parse_field_path("created_by").unwrap();
        assert_eq!(segments, vec![PathSegment::Field("created_by".to_string())]);
    }

    #[test]
    fn test_parse_field_path_empty_string() {
        let result = FieldEvaluator::parse_field_path("");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Empty field path"));
    }

    #[test]
    fn test_extract_simple_field() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "status").unwrap();
        assert_eq!(result, json!("in_progress"));
    }

    #[test]
    fn test_extract_array_field() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "tasks").unwrap();
        assert_eq!(result, json!(["task1", "task2", "task3"]));
    }

    #[test]
    fn test_extract_array_element() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "tasks[0]").unwrap();
        assert_eq!(result, json!("task1"));

        let result = FieldEvaluator::extract_field_value(&json, "tasks[2]").unwrap();
        assert_eq!(result, json!("task3"));
    }

    #[test]
    fn test_extract_nested_field() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "metadata.created_by").unwrap();
        assert_eq!(result, json!("user_123"));

        let result = FieldEvaluator::extract_field_value(&json, "metadata.priority").unwrap();
        assert_eq!(result, json!("high"));
    }

    #[test]
    fn test_extract_nested_array_element() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "metadata.tags[0]").unwrap();
        assert_eq!(result, json!("urgent"));

        let result = FieldEvaluator::extract_field_value(&json, "metadata.tags[1]").unwrap();
        assert_eq!(result, json!("backend"));
    }

    #[test]
    fn test_extract_deep_nested_field() {
        let json = get_test_json();
        let result =
            FieldEvaluator::extract_field_value(&json, "metadata.nested.deep.value").unwrap();
        assert_eq!(result, json!("found_it"));
    }

    #[test]
    fn test_extract_numeric_field() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "counts.total").unwrap();
        assert_eq!(result, json!(42));

        let result = FieldEvaluator::extract_field_value(&json, "counts.completed").unwrap();
        assert_eq!(result, json!(15));
    }

    #[test]
    fn test_extract_empty_array() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "empty_array").unwrap();
        assert_eq!(result, json!([]));
    }

    #[test]
    fn test_extract_single_item_array() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "single_item[0]").unwrap();
        assert_eq!(result, json!("only_one"));
    }

    #[test]
    fn test_extract_nonexistent_field() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "nonexistent");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Field 'nonexistent' not found"));
    }

    #[test]
    fn test_extract_nonexistent_nested_field() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "metadata.nonexistent");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Field 'nonexistent' not found"));
    }

    #[test]
    fn test_extract_array_index_out_of_bounds() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "tasks[99]");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Index 99 not found"));
    }

    #[test]
    fn test_extract_array_index_on_non_array() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "status[0]");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Index 0 not found"));
    }

    #[test]
    fn test_extract_field_on_array_element() {
        let json = json!({
            "users": [
                {"name": "Alice", "age": 30},
                {"name": "Bob", "age": 25}
            ]
        });

        let result = FieldEvaluator::extract_field_value(&json, "users[0].name").unwrap();
        assert_eq!(result, json!("Alice"));

        let result = FieldEvaluator::extract_field_value(&json, "users[1].age").unwrap();
        assert_eq!(result, json!(25));
    }

    #[test]
    fn test_structured_task_output_scenarios() {
        // Test scenarios based on your StructuredTaskOutput example
        let json = json!({
            "tasks": ["setup_database", "create_api", "write_tests"],
            "status": "in_progress"
        });

        // Test extracting the tasks array
        let tasks = FieldEvaluator::extract_field_value(&json, "tasks").unwrap();
        assert!(tasks.is_array());
        assert_eq!(tasks.as_array().unwrap().len(), 3);

        // Test extracting individual task items
        let first_task = FieldEvaluator::extract_field_value(&json, "tasks[0]").unwrap();
        assert_eq!(first_task, json!("setup_database"));

        // Test extracting status
        let status = FieldEvaluator::extract_field_value(&json, "status").unwrap();
        assert_eq!(status, json!("in_progress"));
    }

    #[test]
    fn test_real_world_llm_response_structure() {
        // Test with a more complex LLM response structure
        let json = json!({
            "analysis": {
                "sentiment": "positive",
                "confidence": 0.85,
                "keywords": ["innovation", "growth", "success"]
            },
            "recommendations": [
                {
                    "action": "increase_investment",
                    "priority": "high",
                    "estimated_impact": 0.75
                },
                {
                    "action": "expand_team",
                    "priority": "medium",
                    "estimated_impact": 0.60
                }
            ],
            "summary": "Overall positive outlook with strong growth potential"
        });

        // Test nested object extraction
        let sentiment = FieldEvaluator::extract_field_value(&json, "analysis.sentiment").unwrap();
        assert_eq!(sentiment, json!("positive"));

        // Test array of objects
        let first_action =
            FieldEvaluator::extract_field_value(&json, "recommendations[0].action").unwrap();
        assert_eq!(first_action, json!("increase_investment"));

        // Test numeric extraction
        let confidence = FieldEvaluator::extract_field_value(&json, "analysis.confidence").unwrap();
        assert_eq!(confidence, json!(0.85));

        // Test array element extraction
        let first_keyword =
            FieldEvaluator::extract_field_value(&json, "analysis.keywords[0]").unwrap();
        assert_eq!(first_keyword, json!("innovation"));
    }
}
