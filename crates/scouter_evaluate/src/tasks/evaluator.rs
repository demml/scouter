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

        let expected = Self::resolve_expected_value(json_value, assertion.expected_value())?;

        let comparable_actual =
            match Self::transform_for_comparison(actual_value, assertion.operator()) {
                Ok(val) => val,
                Err(err) => {
                    // return assertion result failure with error message
                    return Ok(AssertionResult::new(
                        false,
                        (*actual_value).clone(),
                        format!(
                            "✗ Assertion '{}' failed during transformation: {}",
                            assertion.id(),
                            err
                        ),
                        expected.clone(),
                    ));
                }
            };

        let passed = Self::compare_values(&comparable_actual, assertion.operator(), expected)?;
        let messages = if passed {
            format!("✓ Assertion '{}' passed", assertion.id())
        } else {
            format!(
                "✗ Assertion '{}' failed: expected {}, got {}",
                assertion.id(),
                serde_json::to_string(expected).unwrap_or_default(),
                serde_json::to_string(&comparable_actual).unwrap_or_default()
            )
        };

        let assertion_result =
            AssertionResult::new(passed, (*actual_value).clone(), messages, expected.clone());

        Ok(assertion_result)
    }

    fn resolve_expected_value<'a>(
        context: &'a Value,
        expected: &'a Value,
    ) -> Result<&'a Value, EvaluationError> {
        match expected {
            Value::String(s) if s.starts_with("${") && s.ends_with("}") => {
                // Extract field path from template: "${field.path}" -> "field.path"
                let field_path = &s[2..s.len() - 1];
                let resolved = FieldEvaluator::extract_field_value(context, field_path)?;
                Ok(resolved)
            }
            _ => Ok(expected),
        }
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
            // Only HasLength should convert to length
            ComparisonOperator::HasLengthEqual
            | ComparisonOperator::HasLengthGreaterThan
            | ComparisonOperator::HasLengthLessThan
            | ComparisonOperator::HasLengthGreaterThanOrEqual
            | ComparisonOperator::HasLengthLessThanOrEqual => {
                if let Some(len) = value.to_length() {
                    Ok(Value::Number(len.into()))
                } else {
                    Err(EvaluationError::CannotGetLength(format!("{:?}", value)))
                }
            }
            // Numeric comparisons should require actual numbers
            ComparisonOperator::LessThan
            | ComparisonOperator::LessThanOrEqual
            | ComparisonOperator::GreaterThan
            | ComparisonOperator::GreaterThanOrEqual => {
                if value.is_number() {
                    Ok(value.clone())
                } else {
                    Err(EvaluationError::CannotCompareNonNumericValues)
                }
            }
            // All other operators pass through unchanged
            _ => Ok(value.clone()),
        }
    }

    fn compare_values(
        actual: &Value,
        operator: &ComparisonOperator,
        expected: &Value,
    ) -> Result<bool, EvaluationError> {
        match operator {
            // Existing operators
            ComparisonOperator::Equals => Ok(actual == expected),
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
            ComparisonOperator::HasLengthEqual => {
                Self::compare_numeric(actual, expected, |a, b| a == b)
            }
            ComparisonOperator::HasLengthGreaterThan => {
                Self::compare_numeric(actual, expected, |a, b| a > b)
            }
            ComparisonOperator::HasLengthLessThan => {
                Self::compare_numeric(actual, expected, |a, b| a < b)
            }
            ComparisonOperator::HasLengthGreaterThanOrEqual => {
                Self::compare_numeric(actual, expected, |a, b| a >= b)
            }
            ComparisonOperator::HasLengthLessThanOrEqual => {
                Self::compare_numeric(actual, expected, |a, b| a <= b)
            }
            ComparisonOperator::Contains => Self::check_contains(actual, expected),
            ComparisonOperator::NotContains => Ok(!Self::check_contains(actual, expected)?),
            ComparisonOperator::StartsWith => Self::check_starts_with(actual, expected),
            ComparisonOperator::EndsWith => Self::check_ends_with(actual, expected),
            ComparisonOperator::Matches => Self::check_regex_match(actual, expected),

            // Type Validation Operators
            ComparisonOperator::IsNumeric => Ok(actual.is_number()),
            ComparisonOperator::IsString => Ok(actual.is_string()),
            ComparisonOperator::IsBoolean => Ok(actual.is_boolean()),
            ComparisonOperator::IsNull => Ok(actual.is_null()),
            ComparisonOperator::IsArray => Ok(actual.is_array()),
            ComparisonOperator::IsObject => Ok(actual.is_object()),

            // Pattern & Format Validators
            ComparisonOperator::IsEmail => Self::check_is_email(actual),
            ComparisonOperator::IsUrl => Self::check_is_url(actual),
            ComparisonOperator::IsUuid => Self::check_is_uuid(actual),
            ComparisonOperator::IsIso8601 => Self::check_is_iso8601(actual),
            ComparisonOperator::IsJson => Self::check_is_json(actual),
            ComparisonOperator::MatchesRegex => Self::check_regex_match(actual, expected),

            // Numeric Range Operators
            ComparisonOperator::InRange => Self::check_in_range(actual, expected),
            ComparisonOperator::NotInRange => Ok(!Self::check_in_range(actual, expected)?),
            ComparisonOperator::IsPositive => Self::check_is_positive(actual),
            ComparisonOperator::IsNegative => Self::check_is_negative(actual),
            ComparisonOperator::IsZero => Self::check_is_zero(actual),

            // Collection/Array Operators
            ComparisonOperator::ContainsAll => Self::check_contains_all(actual, expected),
            ComparisonOperator::ContainsAny => Self::check_contains_any(actual, expected),
            ComparisonOperator::ContainsNone => Self::check_contains_none(actual, expected),
            ComparisonOperator::IsEmpty => Self::check_is_empty(actual),
            ComparisonOperator::IsNotEmpty => Ok(!Self::check_is_empty(actual)?),
            ComparisonOperator::HasUniqueItems => Self::check_has_unique_items(actual),
            ComparisonOperator::SequenceMatches => Self::check_sequence_matches(actual, expected),

            // String Operators
            ComparisonOperator::IsAlphabetic => Self::check_is_alphabetic(actual),
            ComparisonOperator::IsAlphanumeric => Self::check_is_alphanumeric(actual),
            ComparisonOperator::IsLowerCase => Self::check_is_lowercase(actual),
            ComparisonOperator::IsUpperCase => Self::check_is_uppercase(actual),
            ComparisonOperator::ContainsWord => Self::check_contains_word(actual, expected),

            // Comparison with Tolerance
            ComparisonOperator::ApproximatelyEquals => {
                Self::check_approximately_equals(actual, expected)
            }
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

    // Pattern & Format Validation Helpers
    fn check_is_email(actual: &Value) -> Result<bool, EvaluationError> {
        match actual {
            Value::String(s) => {
                let email_regex = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$")
                    .map_err(EvaluationError::RegexError)?;
                Ok(email_regex.is_match(s))
            }
            _ => Err(EvaluationError::InvalidEmailOperation),
        }
    }

    fn check_is_url(actual: &Value) -> Result<bool, EvaluationError> {
        match actual {
            Value::String(s) => {
                let url_regex = Regex::new(
                    r"^https?://[a-zA-Z0-9][a-zA-Z0-9-]*(\.[a-zA-Z0-9][a-zA-Z0-9-]*)*(/.*)?$",
                )
                .map_err(EvaluationError::RegexError)?;
                Ok(url_regex.is_match(s))
            }
            _ => Err(EvaluationError::InvalidUrlOperation),
        }
    }

    fn check_is_uuid(actual: &Value) -> Result<bool, EvaluationError> {
        match actual {
            Value::String(s) => {
                let uuid_regex = Regex::new(
                    r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$"
                ).map_err(EvaluationError::RegexError)?;
                Ok(uuid_regex.is_match(s))
            }
            _ => Err(EvaluationError::InvalidUuidOperation),
        }
    }

    fn check_is_iso8601(actual: &Value) -> Result<bool, EvaluationError> {
        match actual {
            Value::String(s) => {
                // ISO 8601 date-time format
                let iso_regex = Regex::new(
                    r"^\d{4}-\d{2}-\d{2}(T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})?)?$",
                )
                .map_err(EvaluationError::RegexError)?;
                Ok(iso_regex.is_match(s))
            }
            _ => Err(EvaluationError::InvalidIso8601Operation),
        }
    }

    fn check_is_json(actual: &Value) -> Result<bool, EvaluationError> {
        match actual {
            Value::String(s) => Ok(serde_json::from_str::<Value>(s).is_ok()),
            _ => Err(EvaluationError::InvalidJsonOperation),
        }
    }

    // Numeric Range Helpers
    fn check_in_range(actual: &Value, expected: &Value) -> Result<bool, EvaluationError> {
        let actual_num = actual
            .as_numeric()
            .ok_or(EvaluationError::CannotCompareNonNumericValues)?;

        match expected {
            Value::Array(range) if range.len() == 2 => {
                let min = range[0]
                    .as_numeric()
                    .ok_or(EvaluationError::InvalidRangeFormat)?;
                let max = range[1]
                    .as_numeric()
                    .ok_or(EvaluationError::InvalidRangeFormat)?;
                Ok(actual_num >= min && actual_num <= max)
            }
            _ => Err(EvaluationError::InvalidRangeFormat),
        }
    }

    fn check_sequence_matches(actual: &Value, expected: &Value) -> Result<bool, EvaluationError> {
        match (actual, expected) {
            (Value::Array(actual_arr), Value::Array(expected_arr)) => {
                Ok(actual_arr == expected_arr)
            }
            _ => Err(EvaluationError::InvalidSequenceMatchesOperation),
        }
    }

    fn check_is_positive(actual: &Value) -> Result<bool, EvaluationError> {
        let num = actual
            .as_numeric()
            .ok_or(EvaluationError::CannotCompareNonNumericValues)?;
        Ok(num > 0.0)
    }

    fn check_is_negative(actual: &Value) -> Result<bool, EvaluationError> {
        let num = actual
            .as_numeric()
            .ok_or(EvaluationError::CannotCompareNonNumericValues)?;
        Ok(num < 0.0)
    }

    fn check_is_zero(actual: &Value) -> Result<bool, EvaluationError> {
        let num = actual
            .as_numeric()
            .ok_or(EvaluationError::CannotCompareNonNumericValues)?;
        Ok(num == 0.0)
    }

    // Collection/Array Helpers
    fn check_contains_all(actual: &Value, expected: &Value) -> Result<bool, EvaluationError> {
        match (actual, expected) {
            (Value::Array(arr), Value::Array(required)) => {
                Ok(required.iter().all(|item| arr.contains(item)))
            }
            _ => Err(EvaluationError::InvalidContainsAllOperation),
        }
    }

    fn check_contains_any(actual: &Value, expected: &Value) -> Result<bool, EvaluationError> {
        match (actual, expected) {
            (Value::Array(arr), Value::Array(candidates)) => {
                Ok(candidates.iter().any(|item| arr.contains(item)))
            }
            (Value::String(s), Value::Array(keywords)) => Ok(keywords.iter().any(|keyword| {
                if let Value::String(kw) = keyword {
                    s.contains(kw)
                } else {
                    false
                }
            })),
            _ => Err(EvaluationError::InvalidContainsAnyOperation),
        }
    }

    fn check_contains_none(actual: &Value, expected: &Value) -> Result<bool, EvaluationError> {
        match (actual, expected) {
            (Value::Array(arr), Value::Array(forbidden)) => {
                Ok(!forbidden.iter().any(|item| arr.contains(item)))
            }
            _ => Err(EvaluationError::InvalidContainsNoneOperation),
        }
    }

    fn check_is_empty(actual: &Value) -> Result<bool, EvaluationError> {
        match actual {
            Value::String(s) => Ok(s.is_empty()),
            Value::Null => Ok(true),
            Value::Array(arr) => Ok(arr.is_empty()),
            Value::Object(obj) => Ok(obj.is_empty()),
            _ => Err(EvaluationError::InvalidEmptyOperation),
        }
    }

    fn check_has_unique_items(actual: &Value) -> Result<bool, EvaluationError> {
        match actual {
            Value::Array(arr) => {
                let mut seen = std::collections::HashSet::new();
                for item in arr {
                    let json_str = serde_json::to_string(item)
                        .map_err(|_| EvaluationError::InvalidUniqueItemsOperation)?;
                    if !seen.insert(json_str) {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Err(EvaluationError::InvalidUniqueItemsOperation),
        }
    }

    // String Helpers
    fn check_is_alphabetic(actual: &Value) -> Result<bool, EvaluationError> {
        match actual {
            Value::String(s) => Ok(s.chars().all(|c| c.is_alphabetic())),
            _ => Err(EvaluationError::InvalidAlphabeticOperation),
        }
    }

    fn check_is_alphanumeric(actual: &Value) -> Result<bool, EvaluationError> {
        match actual {
            Value::String(s) => Ok(s.chars().all(|c| c.is_alphanumeric())),
            _ => Err(EvaluationError::InvalidAlphanumericOperation),
        }
    }

    fn check_is_lowercase(actual: &Value) -> Result<bool, EvaluationError> {
        match actual {
            Value::String(s) => {
                let has_letters = s.chars().any(|c| c.is_alphabetic());
                if !has_letters {
                    return Err(EvaluationError::InvalidCaseOperation);
                }
                Ok(s.chars()
                    .filter(|c| c.is_alphabetic())
                    .all(|c| c.is_lowercase()))
            }
            _ => Err(EvaluationError::InvalidCaseOperation),
        }
    }

    fn check_is_uppercase(actual: &Value) -> Result<bool, EvaluationError> {
        match actual {
            Value::String(s) => {
                let has_letters = s.chars().any(|c| c.is_alphabetic());
                if !has_letters {
                    return Err(EvaluationError::InvalidCaseOperation);
                }
                Ok(s.chars()
                    .filter(|c| c.is_alphabetic())
                    .all(|c| c.is_uppercase()))
            }
            _ => Err(EvaluationError::InvalidCaseOperation),
        }
    }

    fn check_contains_word(actual: &Value, expected: &Value) -> Result<bool, EvaluationError> {
        match (actual, expected) {
            (Value::String(s), Value::String(word)) => {
                let word_regex = Regex::new(&format!(r"\b{}\b", regex::escape(word)))
                    .map_err(EvaluationError::RegexError)?;
                Ok(word_regex.is_match(s))
            }
            _ => Err(EvaluationError::InvalidContainsWordOperation),
        }
    }

    // Tolerance Comparison Helper
    fn check_approximately_equals(
        actual: &Value,
        expected: &Value,
    ) -> Result<bool, EvaluationError> {
        match expected {
            Value::Array(arr) if arr.len() == 2 => {
                let actual_num = actual
                    .as_numeric()
                    .ok_or(EvaluationError::CannotCompareNonNumericValues)?;
                let expected_num = arr[0]
                    .as_numeric()
                    .ok_or(EvaluationError::InvalidToleranceFormat)?;
                let tolerance = arr[1]
                    .as_numeric()
                    .ok_or(EvaluationError::InvalidToleranceFormat)?;

                Ok((actual_num - expected_num).abs() <= tolerance)
            }
            _ => Err(EvaluationError::InvalidToleranceFormat),
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
            operator: ComparisonOperator::Equals,
            expected_value: Value::String("high".to_string()),
            description: Some("Check if priority is high".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
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
            result: None,
            condition: false,
        }
    }

    fn length_assertion() -> AssertionTask {
        AssertionTask {
            id: "tasks_length".to_string(),
            field_path: Some("tasks".to_string()),
            operator: ComparisonOperator::HasLengthEqual,
            expected_value: Value::Number(3.into()),
            description: Some("There should be 3 tasks".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        }
    }

    fn length_assertion_greater() -> AssertionTask {
        AssertionTask {
            id: "tasks_length_gte".to_string(),
            field_path: Some("tasks".to_string()),
            operator: ComparisonOperator::HasLengthGreaterThanOrEqual,
            expected_value: Value::Number(2.into()),
            description: Some("There should be more than 2 tasks".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        }
    }

    fn length_assertion_less() -> AssertionTask {
        AssertionTask {
            id: "tasks_length_lte".to_string(),
            field_path: Some("tasks".to_string()),
            operator: ComparisonOperator::HasLengthLessThanOrEqual,
            expected_value: Value::Number(5.into()),
            description: Some("There should be less than 5 tasks".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
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
            result: None,
            condition: false,
        }
    }

    fn not_equal_assertion() -> AssertionTask {
        AssertionTask {
            id: "status_not_equal".to_string(),
            field_path: Some("status".to_string()),
            operator: ComparisonOperator::NotEqual,
            expected_value: Value::String("completed".to_string()),
            description: Some("Status should not be completed".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        }
    }

    fn greater_than_assertion() -> AssertionTask {
        AssertionTask {
            id: "total_greater".to_string(),
            field_path: Some("counts.total".to_string()),
            operator: ComparisonOperator::GreaterThan,
            expected_value: Value::Number(40.into()),
            description: Some("Total should be greater than 40".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        }
    }

    fn less_than_assertion() -> AssertionTask {
        AssertionTask {
            id: "completed_less".to_string(),
            field_path: Some("counts.completed".to_string()),
            operator: ComparisonOperator::LessThan,
            expected_value: Value::Number(20.into()),
            description: Some("Completed should be less than 20".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        }
    }

    fn not_contains_assertion() -> AssertionTask {
        AssertionTask {
            id: "tags_not_contains".to_string(),
            field_path: Some("metadata.tags".to_string()),
            operator: ComparisonOperator::NotContains,
            expected_value: Value::String("frontend".to_string()),
            description: Some("Tags should not contain 'frontend'".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        }
    }

    fn starts_with_assertion() -> AssertionTask {
        AssertionTask {
            id: "status_starts_with".to_string(),
            field_path: Some("status".to_string()),
            operator: ComparisonOperator::StartsWith,
            expected_value: Value::String("in_".to_string()),
            description: Some("Status should start with 'in_'".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        }
    }

    fn ends_with_assertion() -> AssertionTask {
        AssertionTask {
            id: "status_ends_with".to_string(),
            field_path: Some("status".to_string()),
            operator: ComparisonOperator::EndsWith,
            expected_value: Value::String("_progress".to_string()),
            description: Some("Status should end with '_progress'".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
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

    #[test]
    fn test_assertion_equals_pass() {
        let json = get_test_json();
        let assertion = priority_assertion();
        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(result.passed);
        assert_eq!(result.actual, json!("high"));
        assert!(result.message.contains("passed"));
    }

    #[test]
    fn test_assertion_equals_fail() {
        let json = get_test_json();
        let mut assertion = priority_assertion();
        assertion.expected_value = Value::String("low".to_string());

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(!result.passed);
        assert_eq!(result.actual, json!("high"));
        assert!(result.message.contains("failed"));
    }

    #[test]
    fn test_assertion_not_equal_pass() {
        let json = get_test_json();
        let assertion = not_equal_assertion();
        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(result.passed);
        assert_eq!(result.actual, json!("in_progress"));
    }

    #[test]
    fn test_assertion_not_equal_fail() {
        let json = get_test_json();
        let mut assertion = not_equal_assertion();
        assertion.expected_value = Value::String("in_progress".to_string());

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(!result.passed);
    }

    #[test]
    fn test_assertion_greater_than_pass() {
        let json = get_test_json();
        let assertion = greater_than_assertion();
        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(result.passed);
        assert_eq!(result.actual, json!(42));
    }

    #[test]
    fn test_assertion_greater_than_fail() {
        let json = get_test_json();
        let mut assertion = greater_than_assertion();
        assertion.expected_value = Value::Number(50.into());

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(!result.passed);
    }

    #[test]
    fn test_assertion_greater_than_or_equal_pass() {
        let json = get_test_json();
        let assertion = length_assertion_greater();
        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(result.passed);
    }

    #[test]
    fn test_assertion_greater_than_or_equal_equal_case() {
        let json = get_test_json();
        let mut assertion = length_assertion_greater();
        assertion.expected_value = Value::Number(3.into());

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(result.passed);
    }

    #[test]
    fn test_assertion_less_than_pass() {
        let json = get_test_json();
        let assertion = less_than_assertion();
        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(result.passed);
        assert_eq!(result.actual, json!(15));
    }

    #[test]
    fn test_assertion_less_than_fail() {
        let json = get_test_json();
        let mut assertion = less_than_assertion();
        assertion.expected_value = Value::Number(10.into());

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(!result.passed);
    }

    #[test]
    fn test_assertion_less_than_or_equal_pass() {
        let json = get_test_json();
        let assertion = length_assertion_less();
        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(result.passed);
    }

    #[test]
    fn test_assertion_less_than_or_equal_equal_case() {
        let json = get_test_json();
        let mut assertion = length_assertion_less();
        assertion.expected_value = Value::Number(3.into());

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(result.passed);
    }

    #[test]
    fn test_assertion_has_length_pass() {
        let json = get_test_json();
        let assertion = length_assertion();
        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(result.passed);
    }

    #[test]
    fn test_assertion_has_length_fail() {
        let json = get_test_json();
        let mut assertion = length_assertion();
        assertion.expected_value = Value::Number(5.into());

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(!result.passed);
    }

    #[test]
    fn test_assertion_has_length_string() {
        let json = json!({"name": "test_user"});
        let assertion = AssertionTask {
            id: "name_length".to_string(),
            field_path: Some("name".to_string()),
            operator: ComparisonOperator::HasLengthEqual,
            expected_value: Value::Number(9.into()),
            description: Some("Name should have 9 characters".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(result.passed);
    }

    #[test]
    fn test_assertion_contains_array_pass() {
        let json = get_test_json();
        let assertion = contains_assertion();
        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(result.passed);
    }

    #[test]
    fn test_assertion_contains_array_fail() {
        let json = get_test_json();
        let mut assertion = contains_assertion();
        assertion.expected_value = Value::String("frontend".to_string());

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(!result.passed);
    }

    #[test]
    fn test_assertion_contains_string_pass() {
        let json = get_test_json();
        let assertion = AssertionTask {
            id: "status_contains_prog".to_string(),
            field_path: Some("status".to_string()),
            operator: ComparisonOperator::Contains,
            expected_value: Value::String("progress".to_string()),
            description: Some("Status should contain 'progress'".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(result.passed);
    }

    #[test]
    fn test_assertion_not_contains_pass() {
        let json = get_test_json();
        let assertion = not_contains_assertion();
        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(result.passed);
    }

    #[test]
    fn test_assertion_not_contains_fail() {
        let json = get_test_json();
        let mut assertion = not_contains_assertion();
        assertion.expected_value = Value::String("backend".to_string());

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(!result.passed);
    }

    #[test]
    fn test_assertion_starts_with_pass() {
        let json = get_test_json();
        let assertion = starts_with_assertion();
        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(result.passed);
    }

    #[test]
    fn test_assertion_starts_with_fail() {
        let json = get_test_json();
        let mut assertion = starts_with_assertion();
        assertion.expected_value = Value::String("completed".to_string());

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(!result.passed);
    }

    #[test]
    fn test_assertion_ends_with_pass() {
        let json = get_test_json();
        let assertion = ends_with_assertion();
        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(result.passed);
    }

    #[test]
    fn test_assertion_ends_with_fail() {
        let json = get_test_json();
        let mut assertion = ends_with_assertion();
        assertion.expected_value = Value::String("_pending".to_string());

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(!result.passed);
    }

    #[test]
    fn test_assertion_matches_pass() {
        let json = get_test_json();
        let assertion = match_assertion();
        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(result.passed);
    }

    #[test]
    fn test_assertion_matches_fail() {
        let json = get_test_json();
        let mut assertion = match_assertion();
        assertion.expected_value = Value::String(r"^completed.*$".to_string());

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(!result.passed);
    }

    #[test]
    fn test_assertion_matches_complex_regex() {
        let json = get_test_json();
        let assertion = AssertionTask {
            id: "user_format".to_string(),
            field_path: Some("metadata.created_by".to_string()),
            operator: ComparisonOperator::Matches,
            expected_value: Value::String(r"^user_\d+$".to_string()),
            description: Some("User ID should match format user_###".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(result.passed);
    }

    #[test]
    fn test_assertion_no_field_path_evaluates_root() {
        let json = json!({"status": "active"});
        let assertion = AssertionTask {
            id: "root_check".to_string(),
            field_path: None,
            operator: ComparisonOperator::Equals,
            expected_value: json!({"status": "active"}),
            description: Some("Check entire root object".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(result.passed);
    }

    #[test]
    fn test_assertion_empty_array_length() {
        let json = get_test_json();
        let assertion = AssertionTask {
            id: "empty_array_length".to_string(),
            field_path: Some("empty_array".to_string()),
            operator: ComparisonOperator::HasLengthEqual,
            expected_value: Value::Number(0.into()),
            description: Some("Empty array should have length 0".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(result.passed);
    }

    #[test]
    fn test_assertion_numeric_comparison_with_floats() {
        let json = json!({"score": 85.5});
        let assertion = AssertionTask {
            id: "score_check".to_string(),
            field_path: Some("score".to_string()),
            operator: ComparisonOperator::GreaterThanOrEqual,
            expected_value: json!(85.0),
            description: Some("Score should be at least 85".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();

        assert!(result.passed);
    }

    #[test]
    fn test_assertion_error_field_not_found() {
        let json = get_test_json();
        let assertion = AssertionTask {
            id: "missing_field".to_string(),
            field_path: Some("nonexistent.field".to_string()),
            operator: ComparisonOperator::Equals,
            expected_value: Value::String("value".to_string()),
            description: Some("Should fail with field not found".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_assertion_error_invalid_regex() {
        let json = get_test_json();
        let assertion = AssertionTask {
            id: "bad_regex".to_string(),
            field_path: Some("status".to_string()),
            operator: ComparisonOperator::Matches,
            expected_value: Value::String("[invalid(".to_string()),
            description: Some("Invalid regex pattern".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion);

        assert!(result.is_err());
    }

    #[test]
    fn test_assertion_error_type_mismatch_starts_with() {
        let json = get_test_json();
        let assertion = AssertionTask {
            id: "type_mismatch".to_string(),
            field_path: Some("counts.total".to_string()),
            operator: ComparisonOperator::StartsWith,
            expected_value: Value::String("4".to_string()),
            description: Some("Cannot use StartsWith on number".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion);

        assert!(result.is_err());
    }

    #[test]
    fn test_assertion_error_type_mismatch_numeric_comparison() {
        let json = json!({"value": "not_a_number"});
        let assertion = AssertionTask {
            id: "numeric_on_string".to_string(),
            field_path: Some("value".to_string()),
            operator: ComparisonOperator::GreaterThan,
            expected_value: Value::Number(10.into()),
            description: Some("Cannot compare string with number".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();
        assert!(!result.passed);
    }

    #[test]
    fn test_is_numeric_pass() {
        let json = json!({"value": 42});
        let assertion = AssertionTask {
            id: "type_check".to_string(),
            field_path: Some("value".to_string()),
            operator: ComparisonOperator::IsNumeric,
            expected_value: Value::Bool(true),
            description: Some("Value should be numeric".to_string()),
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_is_string_pass() {
        let json = json!({"value": "hello"});
        let assertion = AssertionTask {
            id: "type_check".to_string(),
            field_path: Some("value".to_string()),
            operator: ComparisonOperator::IsString,
            expected_value: Value::Bool(true),
            description: None,
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_is_array_pass() {
        let json = json!({"value": [1, 2, 3]});
        let assertion = AssertionTask {
            id: "type_check".to_string(),
            field_path: Some("value".to_string()),
            operator: ComparisonOperator::IsArray,
            expected_value: Value::Bool(true),
            description: None,
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();
        assert!(result.passed);
    }

    // Format Validation Tests
    #[test]
    fn test_is_email_pass() {
        let json = json!({"email": "user@example.com"});
        let assertion = AssertionTask {
            id: "email_check".to_string(),
            field_path: Some("email".to_string()),
            operator: ComparisonOperator::IsEmail,
            expected_value: Value::Bool(true),
            description: None,
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_is_email_fail() {
        let json = json!({"email": "not-an-email"});
        let assertion = AssertionTask {
            id: "email_check".to_string(),
            field_path: Some("email".to_string()),
            operator: ComparisonOperator::IsEmail,
            expected_value: Value::Bool(true),
            description: None,
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();
        assert!(!result.passed);
    }

    #[test]
    fn test_is_url_pass() {
        let json = json!({"url": "https://example.com"});
        let assertion = AssertionTask {
            id: "url_check".to_string(),
            field_path: Some("url".to_string()),
            operator: ComparisonOperator::IsUrl,
            expected_value: Value::Bool(true),
            description: None,
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_is_uuid_pass() {
        let json = json!({"id": "550e8400-e29b-41d4-a716-446655440000"});
        let assertion = AssertionTask {
            id: "uuid_check".to_string(),
            field_path: Some("id".to_string()),
            operator: ComparisonOperator::IsUuid,
            expected_value: Value::Bool(true),
            description: None,
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_is_iso8601_pass() {
        let json = json!({"timestamp": "2024-01-05T10:30:00Z"});
        let assertion = AssertionTask {
            id: "iso_check".to_string(),
            field_path: Some("timestamp".to_string()),
            operator: ComparisonOperator::IsIso8601,
            expected_value: Value::Bool(true),
            description: None,
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_is_json_pass() {
        let json = json!({"data": r#"{"key": "value"}"#});
        let assertion = AssertionTask {
            id: "json_check".to_string(),
            field_path: Some("data".to_string()),
            operator: ComparisonOperator::IsJson,
            expected_value: Value::Bool(true),
            description: None,
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();
        assert!(result.passed);
    }

    // Range Tests
    #[test]
    fn test_in_range_pass() {
        let json = json!({"score": 75});
        let assertion = AssertionTask {
            id: "range_check".to_string(),
            field_path: Some("score".to_string()),
            operator: ComparisonOperator::InRange,
            expected_value: json!([0, 100]),
            description: None,
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_in_range_fail() {
        let json = json!({"score": 150});
        let assertion = AssertionTask {
            id: "range_check".to_string(),
            field_path: Some("score".to_string()),
            operator: ComparisonOperator::InRange,
            expected_value: json!([0, 100]),
            description: None,
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();
        assert!(!result.passed);
    }

    #[test]
    fn test_is_positive_pass() {
        let json = json!({"value": 42});
        let assertion = AssertionTask {
            id: "positive_check".to_string(),
            field_path: Some("value".to_string()),
            operator: ComparisonOperator::IsPositive,
            expected_value: Value::Bool(true),
            description: None,
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_is_negative_pass() {
        let json = json!({"value": -42});
        let assertion = AssertionTask {
            id: "negative_check".to_string(),
            field_path: Some("value".to_string()),
            operator: ComparisonOperator::IsNegative,
            expected_value: Value::Bool(true),
            description: None,
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();
        assert!(result.passed);
    }

    // Collection Tests
    #[test]
    fn test_contains_all_pass() {
        let json = json!({"tags": ["rust", "python", "javascript", "go"]});
        let assertion = AssertionTask {
            id: "contains_all_check".to_string(),
            field_path: Some("tags".to_string()),
            operator: ComparisonOperator::ContainsAll,
            expected_value: json!(["rust", "python"]),
            description: None,
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_contains_any_pass() {
        let json = json!({"tags": ["rust", "python"]});
        let assertion = AssertionTask {
            id: "contains_any_check".to_string(),
            field_path: Some("tags".to_string()),
            operator: ComparisonOperator::ContainsAny,
            expected_value: json!(["python", "java", "c++"]),
            description: None,
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_is_empty_pass() {
        let json = json!({"list": []});
        let assertion = AssertionTask {
            id: "empty_check".to_string(),
            field_path: Some("list".to_string()),
            operator: ComparisonOperator::IsEmpty,
            expected_value: Value::Bool(true),
            description: None,
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_has_unique_items_pass() {
        let json = json!({"items": [1, 2, 3, 4]});
        let assertion = AssertionTask {
            id: "unique_check".to_string(),
            field_path: Some("items".to_string()),
            operator: ComparisonOperator::HasUniqueItems,
            expected_value: Value::Bool(true),
            description: None,
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_has_unique_items_fail() {
        let json = json!({"items": [1, 2, 2, 3]});
        let assertion = AssertionTask {
            id: "unique_check".to_string(),
            field_path: Some("items".to_string()),
            operator: ComparisonOperator::HasUniqueItems,
            expected_value: Value::Bool(true),
            description: None,
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();
        assert!(!result.passed);
    }

    // String Tests
    #[test]
    fn test_is_alphabetic_pass() {
        let json = json!({"text": "HelloWorld"});
        let assertion = AssertionTask {
            id: "alpha_check".to_string(),
            field_path: Some("text".to_string()),
            operator: ComparisonOperator::IsAlphabetic,
            expected_value: Value::Bool(true),
            description: None,
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_is_alphanumeric_pass() {
        let json = json!({"text": "Hello123"});
        let assertion = AssertionTask {
            id: "alphanum_check".to_string(),
            field_path: Some("text".to_string()),
            operator: ComparisonOperator::IsAlphanumeric,
            expected_value: Value::Bool(true),
            description: None,
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_is_lowercase_pass() {
        let json = json!({"text": "hello world"});
        let assertion = AssertionTask {
            id: "lowercase_check".to_string(),
            field_path: Some("text".to_string()),
            operator: ComparisonOperator::IsLowerCase,
            expected_value: Value::Bool(true),
            description: None,
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_is_uppercase_pass() {
        let json = json!({"text": "HELLO WORLD"});
        let assertion = AssertionTask {
            id: "uppercase_check".to_string(),
            field_path: Some("text".to_string()),
            operator: ComparisonOperator::IsUpperCase,
            expected_value: Value::Bool(true),
            description: None,
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_contains_word_pass() {
        let json = json!({"text": "The quick brown fox"});
        let assertion = AssertionTask {
            id: "word_check".to_string(),
            field_path: Some("text".to_string()),
            operator: ComparisonOperator::ContainsWord,
            expected_value: Value::String("quick".to_string()),
            description: None,
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_contains_word_fail() {
        let json = json!({"text": "The quickly brown fox"});
        let assertion = AssertionTask {
            id: "word_check".to_string(),
            field_path: Some("text".to_string()),
            operator: ComparisonOperator::ContainsWord,
            expected_value: Value::String("quick".to_string()),
            description: None,
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();
        assert!(!result.passed);
    }

    // Tolerance Tests
    #[test]
    fn test_approximately_equals_pass() {
        let json = json!({"value": 100.5});
        let assertion = AssertionTask {
            id: "approx_check".to_string(),
            field_path: Some("value".to_string()),
            operator: ComparisonOperator::ApproximatelyEquals,
            expected_value: json!([100.0, 1.0]),
            description: None,
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_approximately_equals_fail() {
        let json = json!({"value": 102.0});
        let assertion = AssertionTask {
            id: "approx_check".to_string(),
            field_path: Some("value".to_string()),
            operator: ComparisonOperator::ApproximatelyEquals,
            expected_value: json!([100.0, 1.0]),
            description: None,
            task_type: EvaluationTaskType::Assertion,
            depends_on: vec![],
            result: None,
            condition: false,
        };

        let result = AssertionEvaluator::evaluate_assertion(&json, &assertion).unwrap();
        assert!(!result.passed);
    }
}
