use crate::error::EvaluationError;
use regex::Regex;
use scouter_types::genai::ValueExt;
use scouter_types::genai::{traits::TaskAccessor, AssertionResult, ComparisonOperator};
use serde_json::Value;
use std::sync::OnceLock;

const REGEX_FIELD_PARSE_PATTERN: &str = r"[a-zA-Z_][a-zA-Z0-9_]*|\[[0-9]+\]";
static PATH_REGEX: OnceLock<Regex> = OnceLock::new();

pub struct FieldEvaluator;

/// Utility for extracting field values from JSON-like structures
/// Supports: "field", "field.subfield", "field[0]", "field[0].subfield"
impl FieldEvaluator {
    /// Extracts the value at the specified field path from the given JSON value
    /// # Arguments
    /// * `json` - The JSON value to extract from
    /// * `field_path` - The dot/bracket notation path to the desired field
    /// # Returns
    /// The extracted JSON value or an EvaluationError if the path is invalid
    pub fn extract_field_value<'a>(
        json: &'a Value,
        field_path: &str,
    ) -> Result<&'a Value, EvaluationError> {
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

        Ok(current_value)
    }

    /// Parses a field path string into segments for navigation
    /// # Arguments
    /// * `path` - The field path string
    /// # Returns
    /// A vector of PathSegment enums representing the parsed path
    fn parse_field_path(path: &str) -> Result<Vec<PathSegment>, EvaluationError> {
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

#[derive(Debug, Clone)]
pub struct AssertionEvaluator;

impl AssertionEvaluator {
    /// Main function for evaluating an assertion
    /// # Arguments
    /// * `json_value` - The JSON value to evaluate against
    /// * `assertion` - The assertion to evaluate
    /// # Returns
    /// An AssertionResult indicating whether the assertion passed or failed
    pub fn evaluate_assertion<T: TaskAccessor>(
        json_value: &Value,
        assertion: &T,
    ) -> Result<AssertionResult, EvaluationError> {
        let actual_value: &Value = if let Some(field_path) = assertion.field_path() {
            FieldEvaluator::extract_field_value(json_value, field_path)?
        } else {
            json_value
        };

        // Transform for comparison (now uses Cow - only clones when necessary)
        let comparable_actual =
            Self::transform_for_comparison(&actual_value, assertion.operator())?;
        let expected = assertion.expected_value();

        // Comparison works with references - no clone
        let passed = Self::compare_values(&comparable_actual, assertion.operator(), expected)?;

        // AssertionResult creation - necessary clones for owned storage
        Ok(AssertionResult {
            id: assertion.id().to_string(),
            passed,
            field_path: assertion.field_path().map(|s| s.to_string()),
            expected: expected.clone(),
            actual: (*actual_value).clone(),
            message: if passed {
                format!("✓ Assertion '{}' passed", assertion.id())
            } else {
                format!(
                    "✗ Assertion '{}' failed: expected {}, got {}",
                    assertion.id(),
                    serde_json::to_string(expected).unwrap_or_default(),
                    serde_json::to_string(&comparable_actual).unwrap_or_default()
                )
            },
        })
    }

    /// Transforms a value based on the comparison operator
    /// This is mainly used to convert array, string and map types that have
    /// length to their length for length-based comparisons
    /// # Arguments
    /// * `value` - The value to transform
    /// * `operator` - The comparison operator
    /// # Returns
    /// The transformed value or an EvaluationError if transformation fails
    fn transform_for_comparison(
        value: &Value,
        operator: &ComparisonOperator,
    ) -> Result<Value, EvaluationError> {
        match operator {
            ComparisonOperator::HasLength
            | ComparisonOperator::LessThan
            | ComparisonOperator::LessThanOrEqual
            | ComparisonOperator::GreaterThan
            | ComparisonOperator::GreaterThanOrEqual => {
                if let Some(len) = value.to_length() {
                    Ok(Value::Number(len.into()))
                } else if value.is_number() {
                    Ok(value.clone()) // Must clone to return owned
                } else {
                    Err(EvaluationError::CannotGetLength(format!("{:?}", value)))
                }
            }
            _ => Ok(value.clone()), // Must clone to return owned
        }
    }

    // All comparison methods work with &Value - no clones needed
    fn compare_values(
        actual: &Value,
        operator: &ComparisonOperator,
        expected: &Value,
    ) -> Result<bool, EvaluationError> {
        match operator {
            ComparisonOperator::Equal => Ok(actual == expected),
            ComparisonOperator::NotEqual => Ok(actual != expected),

            ComparisonOperator::GreaterThan => {
                Self::compare_numeric(actual, expected, |a, b| a > b)
            }
            ComparisonOperator::GreaterThanOrEqual => {
                Self::compare_numeric(actual, expected, |a, b| a >= b)
            }
            ComparisonOperator::LessThan => Self::compare_numeric(actual, expected, |a, b| a < b),
            ComparisonOperator::LessThanOrEqual => {
                Self::compare_numeric(actual, expected, |a, b| a <= b)
            }
            ComparisonOperator::HasLength => Self::compare_numeric(actual, expected, |a, b| a == b),

            ComparisonOperator::Contains => Self::check_contains(actual, expected),
            ComparisonOperator::NotContains => Ok(!Self::check_contains(actual, expected)?),
            ComparisonOperator::StartsWith => Self::check_starts_with(actual, expected),
            ComparisonOperator::EndsWith => Self::check_ends_with(actual, expected),
            ComparisonOperator::Matches => Self::check_regex_match(actual, expected),
        }
    }

    // Comparison helpers - all work with references
    fn compare_numeric<F>(
        actual: &Value,
        expected: &Value,
        comparator: F,
    ) -> Result<bool, EvaluationError>
    where
        F: Fn(f64, f64) -> bool,
    {
        let actual_num = actual
            .as_numeric()
            .ok_or(EvaluationError::CannotCompareNonNumericValues)?;
        let expected_num = expected
            .as_numeric()
            .ok_or(EvaluationError::CannotCompareNonNumericValues)?;

        Ok(comparator(actual_num, expected_num))
    }

    fn check_contains(actual: &Value, expected: &Value) -> Result<bool, EvaluationError> {
        match (actual, expected) {
            (Value::String(s), Value::String(substr)) => Ok(s.contains(substr)),
            (Value::Array(arr), expected_item) => Ok(arr.contains(expected_item)),
            _ => Err(EvaluationError::InvalidContainsOperation),
        }
    }

    fn check_starts_with(actual: &Value, expected: &Value) -> Result<bool, EvaluationError> {
        match (actual, expected) {
            (Value::String(s), Value::String(prefix)) => Ok(s.starts_with(prefix)),
            _ => Err(EvaluationError::InvalidStartsWithOperation),
        }
    }

    fn check_ends_with(actual: &Value, expected: &Value) -> Result<bool, EvaluationError> {
        match (actual, expected) {
            (Value::String(s), Value::String(suffix)) => Ok(s.ends_with(suffix)),
            _ => Err(EvaluationError::InvalidEndsWithOperation),
        }
    }

    fn check_regex_match(actual: &Value, expected: &Value) -> Result<bool, EvaluationError> {
        match (actual, expected) {
            (Value::String(s), Value::String(pattern)) => {
                let regex = Regex::new(pattern)?;
                Ok(regex.is_match(s))
            }
            _ => Err(EvaluationError::InvalidRegexOperation),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use scouter_types::genai::AssertionTask;
    use scouter_types::genai::EvaluationTaskType;
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

    fn priority_assertion() -> AssertionTask {
        AssertionTask {
            id: "priority_check".to_string(),
            field_path: Some("metadata.priority".to_string()),
            operator: ComparisonOperator::Equal,
            expected_value: Value::String("high".to_string()),
            description: Some("Check if priority is high".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
        }
    }

    fn match_assertion() -> AssertionTask {
        AssertionTask {
            id: "status_match".to_string(),
            field_path: Some("status".to_string()),
            operator: ComparisonOperator::Matches,
            expected_value: Value::String(r"^in_.*$".to_string()),
            description: Some("Status should start with 'in_'".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
        }
    }

    fn length_assertion() -> AssertionTask {
        AssertionTask {
            id: "tasks_length".to_string(),
            field_path: Some("tasks".to_string()),
            operator: ComparisonOperator::HasLength,
            expected_value: Value::Number(3.into()),
            description: Some("There should be 3 tasks".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
        }
    }

    fn length_assertion_greater() -> AssertionTask {
        AssertionTask {
            id: "tasks_length_gte".to_string(),
            field_path: Some("tasks".to_string()),
            operator: ComparisonOperator::GreaterThanOrEqual,
            expected_value: Value::Number(2.into()),
            description: Some("There should be more than 2 tasks".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
        }
    }

    fn length_assertion_less() -> AssertionTask {
        AssertionTask {
            id: "tasks_length_lte".to_string(),
            field_path: Some("tasks".to_string()),
            operator: ComparisonOperator::LessThanOrEqual,
            expected_value: Value::Number(5.into()),
            description: Some("There should be less than 5 tasks".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
        }
    }

    fn contains_assertion() -> AssertionTask {
        AssertionTask {
            id: "tags_contains".to_string(),
            field_path: Some("metadata.tags".to_string()),
            operator: ComparisonOperator::Contains,
            expected_value: Value::String("backend".to_string()),
            description: Some("Tags should contain 'backend'".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
        }
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
        assert_eq!(*result, json!("in_progress"));
    }

    #[test]
    fn test_extract_array_field() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "tasks").unwrap();
        assert_eq!(*result, json!(["task1", "task2", "task3"]));
    }

    #[test]
    fn test_extract_array_element() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "tasks[0]").unwrap();
        assert_eq!(*result, json!("task1"));

        let result = FieldEvaluator::extract_field_value(&json, "tasks[2]").unwrap();
        assert_eq!(*result, json!("task3"));
    }

    #[test]
    fn test_extract_nested_field() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "metadata.created_by").unwrap();
        assert_eq!(*result, json!("user_123"));

        let result = FieldEvaluator::extract_field_value(&json, "metadata.priority").unwrap();
        assert_eq!(*result, json!("high"));
    }

    #[test]
    fn test_extract_nested_array_element() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "metadata.tags[0]").unwrap();
        assert_eq!(*result, json!("urgent"));

        let result = FieldEvaluator::extract_field_value(&json, "metadata.tags[1]").unwrap();
        assert_eq!(*result, json!("backend"));
    }

    #[test]
    fn test_extract_deep_nested_field() {
        let json = get_test_json();
        let result =
            FieldEvaluator::extract_field_value(&json, "metadata.nested.deep.value").unwrap();
        assert_eq!(*result, json!("found_it"));
    }

    #[test]
    fn test_extract_numeric_field() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "counts.total").unwrap();
        assert_eq!(*result, json!(42));

        let result = FieldEvaluator::extract_field_value(&json, "counts.completed").unwrap();
        assert_eq!(*result, json!(15));
    }

    #[test]
    fn test_extract_empty_array() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "empty_array").unwrap();
        assert_eq!(*result, json!([]));
    }

    #[test]
    fn test_extract_single_item_array() {
        let json = get_test_json();
        let result = FieldEvaluator::extract_field_value(&json, "single_item[0]").unwrap();
        assert_eq!(*result, json!("only_one"));
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
        assert_eq!(*result, json!("Alice"));

        let result = FieldEvaluator::extract_field_value(&json, "users[1].age").unwrap();
        assert_eq!(*result, json!(25));
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
        assert_eq!(*first_task, json!("setup_database"));

        // Test extracting status
        let status = FieldEvaluator::extract_field_value(&json, "status").unwrap();
        assert_eq!(*status, json!("in_progress"));
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
        assert_eq!(*sentiment, json!("positive"));

        // Test array of objects
        let first_action =
            FieldEvaluator::extract_field_value(&json, "recommendations[0].action").unwrap();
        assert_eq!(*first_action, json!("increase_investment"));

        // Test numeric extraction
        let confidence = FieldEvaluator::extract_field_value(&json, "analysis.confidence").unwrap();
        assert_eq!(*confidence, json!(0.85));
        // Test array element extraction
        let first_keyword =
            FieldEvaluator::extract_field_value(&json, "analysis.keywords[0]").unwrap();
        assert_eq!(*first_keyword, json!("innovation"));
    }
}
