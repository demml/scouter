use crate::error::EvaluationError;
use regex::Regex;
/// Core logic of evaluation trace spans as part of TraceAssertionTask
///
/// use scouter_types::sql::TraceSpan;
use scouter_types::genai::{AggregationType, SpanFilter, SpanStatus, TraceAssertion};
use scouter_types::sql::TraceSpan;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::Arc;
use tracing::debug;

#[derive(Debug, Clone)]
pub struct TraceContextBuilder {
    /// We want to share trace spans across multiple evaluations
    pub(crate) spans: Arc<Vec<TraceSpan>>,
}

impl TraceContextBuilder {
    pub fn new(spans: Arc<Vec<TraceSpan>>) -> Self {
        Self { spans }
    }

    /// Converts trace data into a JSON context that AssertionEvaluator can process
    pub fn build_context(&self, assertion: &TraceAssertion) -> Result<Value, EvaluationError> {
        match assertion {
            TraceAssertion::SpanSequence { span_names } => {
                Ok(json!(self.match_span_sequence(span_names)?))
            }
            TraceAssertion::SpanSet { span_names } => Ok(json!(self.match_span_set(span_names)?)),
            TraceAssertion::SpanCount { filter } => Ok(json!(self.count_spans(filter)?)),
            TraceAssertion::SpanExists { filter } => Ok(json!(self.span_exists(filter)?)),
            TraceAssertion::SpanAttribute {
                filter,
                attribute_key,
            } => self.extract_span_attribute(filter, attribute_key),
            TraceAssertion::SpanDuration { filter } => self.extract_span_duration(filter),
            TraceAssertion::SpanAggregation {
                filter,
                attribute_key,
                aggregation,
            } => self.aggregate_span_attribute(filter, attribute_key, aggregation),
            TraceAssertion::TraceDuration {} => Ok(json!(self.calculate_trace_duration())),
            TraceAssertion::TraceSpanCount {} => Ok(json!(self.spans.len())),
            TraceAssertion::TraceErrorCount {} => Ok(json!(self.count_error_spans())),
            TraceAssertion::TraceServiceCount {} => Ok(json!(self.count_unique_services())),
            TraceAssertion::TraceMaxDepth {} => Ok(json!(self.calculate_max_depth())),
            TraceAssertion::TraceAttribute { attribute_key } => {
                self.extract_trace_attribute(attribute_key)
            }
        }
    }

    // Span filtering logic
    fn filter_spans(&self, filter: &SpanFilter) -> Result<Vec<&TraceSpan>, EvaluationError> {
        let mut filtered = Vec::new();

        for span in self.spans.iter() {
            if self.matches_filter(span, filter)? {
                filtered.push(span);
            }
        }

        debug!(
            "Filtered spans count: {} with filter {:?}",
            filtered.len(),
            filter
        );

        Ok(filtered)
    }

    fn matches_filter(
        &self,
        span: &TraceSpan,
        filter: &SpanFilter,
    ) -> Result<bool, EvaluationError> {
        match filter {
            SpanFilter::ByName { name } => Ok(span.span_name == *name),

            SpanFilter::ByNamePattern { pattern } => {
                let regex = Regex::new(pattern)?;
                Ok(regex.is_match(&span.span_name))
            }

            SpanFilter::WithAttribute { key } => {
                Ok(span.attributes.iter().any(|attr| attr.key == *key))
            }

            SpanFilter::WithAttributeValue { key, value } => {
                Ok(span.attributes.iter().any(|attr| {
                    attr.key == *key && self.attribute_value_matches(&attr.value, &value.0)
                }))
            }

            SpanFilter::WithStatus { status } => {
                Ok(self.map_status_code(span.status_code) == *status)
            }

            SpanFilter::WithDuration { min_ms, max_ms } => {
                let duration_f64 = span.duration_ms as f64;
                let min_ok = min_ms.is_none_or(|min| duration_f64 >= min);
                let max_ok = max_ms.is_none_or(|max| duration_f64 <= max);
                Ok(min_ok && max_ok)
            }

            SpanFilter::And { filters } => {
                for f in filters {
                    if !self.matches_filter(span, f)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }

            SpanFilter::Or { filters } => {
                for f in filters {
                    if self.matches_filter(span, f)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }

            SpanFilter::Sequence { .. } => Err(EvaluationError::InvalidFilter(
                "Sequence filter not applicable to individual spans".to_string(),
            )),
        }
    }

    /// Get ordered list of span names
    fn match_span_sequence(&self, span_names: &[String]) -> Result<bool, EvaluationError> {
        let executed_names = self.get_ordered_span_names()?;
        Ok(executed_names == span_names)
    }

    /// Get unique set of span names. Order does not matter.
    fn match_span_set(&self, span_names: &[String]) -> Result<bool, EvaluationError> {
        let unique_names: HashSet<_> = self.spans.iter().map(|s| s.span_name.clone()).collect();
        for name in span_names {
            if !unique_names.contains(name) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn count_spans(&self, filter: &SpanFilter) -> Result<usize, EvaluationError> {
        match filter {
            SpanFilter::Sequence { names } => self.count_sequence_occurrences(names),
            _ => Ok(self.filter_spans(filter)?.len()),
        }
    }

    /// Count how many times a specific sequence of span names appears consecutively
    fn count_sequence_occurrences(
        &self,
        target_sequence: &[String],
    ) -> Result<usize, EvaluationError> {
        if target_sequence.is_empty() {
            return Ok(0);
        }

        let all_span_names = self.get_ordered_span_names()?;

        if all_span_names.len() < target_sequence.len() {
            return Ok(0);
        }

        Ok(all_span_names
            .windows(target_sequence.len())
            .filter(|window| *window == target_sequence)
            .count())
    }

    fn get_ordered_span_names(&self) -> Result<Vec<String>, EvaluationError> {
        let mut ordered_spans: Vec<_> = self.spans.iter().collect();
        ordered_spans.sort_by_key(|s| s.span_order);

        Ok(ordered_spans
            .into_iter()
            .map(|s| s.span_name.clone())
            .collect())
    }

    fn span_exists(&self, filter: &SpanFilter) -> Result<bool, EvaluationError> {
        Ok(!self.filter_spans(filter)?.is_empty())
    }

    fn extract_span_attribute(
        &self,
        filter: &SpanFilter,
        attribute_key: &str,
    ) -> Result<Value, EvaluationError> {
        let filtered_spans = self.filter_spans(filter)?;

        if filtered_spans.is_empty() {
            return Ok(Value::Null);
        }

        let values: Vec<Value> = filtered_spans
            .iter()
            .filter_map(|span| {
                span.attributes
                    .iter()
                    .find(|attr| attr.key == attribute_key)
                    .map(|attr| attr.value.clone())
            })
            .collect();

        if values.len() == 1 {
            Ok(values[0].clone())
        } else {
            Ok(Value::Array(values))
        }
    }

    fn extract_span_duration(&self, filter: &SpanFilter) -> Result<Value, EvaluationError> {
        let filtered_spans = self.filter_spans(filter)?;

        let durations: Vec<i64> = filtered_spans.iter().map(|span| span.duration_ms).collect();

        if durations.len() == 1 {
            Ok(json!(durations[0]))
        } else {
            Ok(json!(durations))
        }
    }

    fn aggregate_span_attribute(
        &self,
        filter: &SpanFilter,
        attribute_key: &str,
        aggregation: &AggregationType,
    ) -> Result<Value, EvaluationError> {
        let filtered_spans = self.filter_spans(filter)?;

        match aggregation {
            AggregationType::Count => {
                let count = filtered_spans
                    .iter()
                    .filter(|span| span.attributes.iter().any(|attr| attr.key == attribute_key))
                    .count();
                Ok(json!(count))
            }
            _ => {
                let values: Vec<f64> = filtered_spans
                    .iter()
                    .filter_map(|span| {
                        span.attributes
                            .iter()
                            .find(|attr| attr.key == attribute_key)
                            .and_then(|attr| attr.value.as_f64())
                    })
                    .collect();

                if values.is_empty() {
                    return Ok(Value::Null);
                }

                let result = match aggregation {
                    AggregationType::Count => unreachable!(),
                    AggregationType::Sum => values.iter().sum(),
                    AggregationType::Average => values.iter().sum::<f64>() / values.len() as f64,
                    AggregationType::Min => values.iter().copied().fold(f64::INFINITY, f64::min),
                    AggregationType::Max => {
                        values.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                    }
                    AggregationType::First => values[0],
                    AggregationType::Last => values[values.len() - 1],
                };

                Ok(json!(result))
            }
        }
    }

    // Trace-level calculations
    fn calculate_trace_duration(&self) -> i64 {
        self.spans.iter().map(|s| s.duration_ms).max().unwrap_or(0)
    }

    fn count_error_spans(&self) -> usize {
        self.spans
            .iter()
            .filter(|s| s.status_code == 2) // Error status
            .count()
    }

    fn count_unique_services(&self) -> usize {
        self.spans
            .iter()
            .map(|s| &s.service_name)
            .collect::<HashSet<_>>()
            .len()
    }

    fn calculate_max_depth(&self) -> i32 {
        self.spans.iter().map(|s| s.depth).max().unwrap_or(0)
    }

    fn extract_trace_attribute(&self, attribute_key: &str) -> Result<Value, EvaluationError> {
        let root_span = self
            .spans
            .iter()
            .find(|s| s.depth == 0)
            .ok_or_else(|| EvaluationError::NoRootSpan)?;

        root_span
            .attributes
            .iter()
            .find(|attr| attr.key == attribute_key)
            .map(|attr| attr.value.clone())
            .ok_or_else(|| EvaluationError::AttributeNotFound(attribute_key.to_string()))
    }

    // Helper methods
    fn map_status_code(&self, code: i32) -> SpanStatus {
        match code {
            0 => SpanStatus::Unset,
            1 => SpanStatus::Ok,
            2 => SpanStatus::Error,
            _ => SpanStatus::Unset,
        }
    }

    fn attribute_value_matches(&self, attr_value: &Value, expected: &Value) -> bool {
        attr_value == expected
    }
}

#[cfg(test)]
mod tests {
    use scouter_types::genai::PyValueWrapper;

    use super::*;

    use scouter_mocks::{
        create_multi_service_trace, create_nested_trace, create_sequence_pattern_trace,
        create_simple_trace, create_trace_with_attributes, create_trace_with_errors,
    };

    #[test]
    fn test_simple_trace_structure() {
        let spans = create_simple_trace();
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].span_name, "root");
        assert_eq!(spans[0].depth, 0);
        assert_eq!(spans[1].parent_span_id, Some("span_0".to_string()));
    }

    #[test]
    fn test_nested_trace_depth() {
        let spans = create_nested_trace();
        let builder = TraceContextBuilder::new(Arc::new(spans));
        assert_eq!(builder.calculate_max_depth(), 2);
    }

    #[test]
    fn test_error_counting() {
        let spans = create_trace_with_errors();
        let builder = TraceContextBuilder::new(Arc::new(spans));
        assert_eq!(builder.count_error_spans(), 1);
    }

    #[test]
    fn test_attribute_filtering() {
        let spans = create_trace_with_attributes();
        let builder = TraceContextBuilder::new(Arc::new(spans));

        let filter = SpanFilter::WithAttribute {
            key: "model".to_string(),
        };

        let result = builder.span_exists(&filter).unwrap();
        assert!(result);
    }

    #[test]
    fn test_sequence_pattern_detection() {
        let spans = create_sequence_pattern_trace();
        let builder = TraceContextBuilder::new(Arc::new(spans));

        let filter = SpanFilter::Sequence {
            names: vec!["call_tool".to_string(), "run_agent".to_string()],
        };

        let count = builder.count_spans(&filter).unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_multi_service_trace() {
        let spans = create_multi_service_trace();
        let builder = TraceContextBuilder::new(Arc::new(spans));
        assert_eq!(builder.count_unique_services(), 3);
    }

    #[test]
    fn test_aggregation_with_numeric_attributes() {
        let spans = create_trace_with_attributes();
        let builder = TraceContextBuilder::new(Arc::new(spans));

        let filter = SpanFilter::WithAttribute {
            key: "tokens.input".to_string(),
        };

        let result = builder
            .aggregate_span_attribute(&filter, "tokens.input", &AggregationType::Sum)
            .unwrap();

        assert_eq!(result, json!(150.0));
    }

    #[test]
    fn test_trace_assertion_span_sequence_evaluation() {
        let spans = create_simple_trace();
        let builder = TraceContextBuilder::new(Arc::new(spans));

        let assertion = TraceAssertion::SpanSequence {
            span_names: vec![
                "root".to_string(),
                "child_1".to_string(),
                "child_2".to_string(),
            ],
        };

        let context = builder.build_context(&assertion).unwrap();
        assert_eq!(context, json!(true));
    }

    #[test]
    fn test_trace_assertion_span_set_evaluation() {
        let spans = create_simple_trace();
        let builder = TraceContextBuilder::new(Arc::new(spans));

        let assertion = TraceAssertion::SpanSet {
            span_names: vec![
                "root".to_string(),
                "child_1".to_string(),
                "child_2".to_string(),
            ],
        };

        let context = builder.build_context(&assertion).unwrap();
        assert_eq!(context, json!(true));
    }

    #[test]
    fn test_trace_assertion_span_count() {
        let spans = create_simple_trace();
        let builder = TraceContextBuilder::new(Arc::new(spans));

        let filter = SpanFilter::ByName {
            name: "child_1".to_string(),
        };

        let assertion = TraceAssertion::SpanCount { filter };

        let context = builder.build_context(&assertion).unwrap();
        assert_eq!(context, json!(1));

        // Test with name pattern
        let filter_pattern = SpanFilter::ByNamePattern {
            pattern: "^child_.*".to_string(),
        };

        let assertion_pattern = TraceAssertion::SpanCount {
            filter: filter_pattern,
        };
        let context_pattern = builder.build_context(&assertion_pattern).unwrap();
        assert_eq!(context_pattern, json!(2));

        // Test span count with attribute filter
        let trace_with_attributes = create_trace_with_attributes();
        let builder_attr = TraceContextBuilder::new(Arc::new(trace_with_attributes));

        let filter_attr = SpanFilter::WithAttribute {
            key: "model".to_string(),
        };

        let assertion_attr = TraceAssertion::SpanCount {
            filter: filter_attr,
        };
        let context_attr = builder_attr.build_context(&assertion_attr).unwrap();
        assert_eq!(context_attr, json!(1));

        // Test span count with attribute value filter
        let filter_attr_value = SpanFilter::WithAttributeValue {
            key: "http.method".to_string(),
            value: PyValueWrapper(json!("POST")),
        };

        let assertion_attr_value = TraceAssertion::SpanCount {
            filter: filter_attr_value,
        };
        let context_attr_value = builder_attr.build_context(&assertion_attr_value).unwrap();
        assert_eq!(context_attr_value, json!(1));

        // test span count with status filter
        let filter_status = SpanFilter::WithStatus {
            status: SpanStatus::Ok,
        };
        let assertion_status = TraceAssertion::SpanCount {
            filter: filter_status,
        };
        let context_status = builder_attr.build_context(&assertion_status).unwrap();
        assert_eq!(context_status, json!(2));

        // test duration filter
        let filter_duration = SpanFilter::WithDuration {
            min_ms: Some(80.0),
            max_ms: Some(120.0),
        };
        let assertion_duration = TraceAssertion::SpanCount {
            filter: filter_duration,
        };
        let context_duration = builder_attr.build_context(&assertion_duration).unwrap();
        assert_eq!(context_duration, json!(1));

        // test complex AND filter
        let filter_and = SpanFilter::And {
            filters: vec![
                SpanFilter::WithAttribute {
                    key: "http.method".to_string(),
                },
                SpanFilter::WithStatus {
                    status: SpanStatus::Ok,
                },
            ],
        };
        let assertion_and = TraceAssertion::SpanCount { filter: filter_and };
        let context_and = builder_attr.build_context(&assertion_and).unwrap();
        assert_eq!(context_and, json!(1));

        // test complex OR filter
        let filter_or = SpanFilter::Or {
            filters: vec![
                SpanFilter::WithAttributeValue {
                    key: "http.method".to_string(),
                    value: PyValueWrapper(json!("GET")),
                },
                SpanFilter::WithAttributeValue {
                    key: "model".to_string(),
                    value: PyValueWrapper(json!("gpt-4")),
                },
            ],
        };
        let assertion_or = TraceAssertion::SpanCount { filter: filter_or };
        let context_or = builder_attr.build_context(&assertion_or).unwrap();
        assert_eq!(context_or, json!(1));
    }

    #[test]
    fn test_span_exists() {
        let spans = create_simple_trace();
        let builder = TraceContextBuilder::new(Arc::new(spans));
        let filter = SpanFilter::ByName {
            name: "child_1".to_string(),
        };
        let assertion = TraceAssertion::SpanExists { filter };
        let context = builder.build_context(&assertion).unwrap();
        assert_eq!(context, json!(true));
    }

    #[test]
    fn test_span_attribute() {
        // test model
        let spans = create_trace_with_attributes();
        let builder = TraceContextBuilder::new(Arc::new(spans));
        let filter = SpanFilter::ByName {
            name: "api_call".to_string(),
        };
        let assertion = TraceAssertion::SpanAttribute {
            filter,
            attribute_key: "model".to_string(),
        };
        let context = builder.build_context(&assertion).unwrap();
        assert_eq!(context, json!("gpt-4"));

        // check response
        let spans = create_trace_with_attributes();
        let builder = TraceContextBuilder::new(Arc::new(spans));
        let filter = SpanFilter::ByName {
            name: "api_call".to_string(),
        };
        let assertion = TraceAssertion::SpanAttribute {
            filter,
            attribute_key: "response".to_string(),
        };
        let context = builder.build_context(&assertion).unwrap();
        assert_eq!(context, json!({"success": true, "data": {"id": 12345}}));
    }

    #[test]
    fn test_span_attribute_aggregation() {
        let spans = create_trace_with_attributes();
        let builder = TraceContextBuilder::new(Arc::new(spans));
        let filter = SpanFilter::ByName {
            name: "api_call".to_string(),
        };
        let assertion = TraceAssertion::SpanAggregation {
            filter,
            attribute_key: "tokens.output".to_string(),
            aggregation: AggregationType::Sum,
        };
        let context = builder.build_context(&assertion).unwrap();
        assert_eq!(context, json!(300.0));
    }

    /// check common sequence patterns
    #[test]
    fn test_sequence_pattern_counting() {
        // count how often "call_tool" followed by "run_agent" occurs
        let spans = create_sequence_pattern_trace();
        let builder = TraceContextBuilder::new(Arc::new(spans));
        let filter = SpanFilter::Sequence {
            names: vec!["call_tool".to_string(), "run_agent".to_string()],
        };
        let assertion = TraceAssertion::SpanCount { filter };
        let context = builder.build_context(&assertion).unwrap();
        assert_eq!(context, json!(2));

        // count how often "call_tool" occurs
        let spans = create_sequence_pattern_trace();
        let builder = TraceContextBuilder::new(Arc::new(spans));
        let filter = SpanFilter::ByName {
            name: "call_tool".to_string(),
        };
        let assertion = TraceAssertion::SpanCount { filter };
        let context = builder.build_context(&assertion).unwrap();
        assert_eq!(context, json!(2));
    }
}
