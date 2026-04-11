use crate::error::EvaluationError;
use crate::tasks::evaluator::FieldEvaluator;
use potato_head::{ChatResponse, Provider};
use scouter_types::agent::AgentAssertion;
use serde_json::{json, Value};
use tracing::error;

/// Builds evaluation context from vendor LLM request/response data.
/// Normalizes vendor-specific formats into a standard structure for assertion evaluation.
#[derive(Debug, Clone)]
pub struct AgentContextBuilder<'a> {
    response: ChatResponse,
    raw: &'a Value,
}

impl<'a> AgentContextBuilder<'a> {
    /// Build an AgentContextBuilder from raw context JSON.
    ///
    /// Extraction strategy (priority order):
    /// 1. Pre-normalized — if top-level keys match our standard shape
    /// 2. OpenAI format — choices[].message.tool_calls, usage, model
    /// 3. Anthropic format — content[] with ToolUseBlock, usage, model
    /// 4. Google/Gemini format — candidates[].content.parts[] with function_call
    /// 5. Fallback — walk JSON tree for known patterns
    pub fn from_context(
        context: &'a Value,
        provider: Option<&Provider>,
        context_path: Option<&str>,
    ) -> Result<Self, EvaluationError> {
        let raw: &'a Value = if let Some(path) = context_path {
            FieldEvaluator::extract_field_value(context, path)?
        } else {
            context
        };
        let response = ChatResponse::from_response_value(raw.clone(), provider).map_err(|e| {
            error!("Failed to parse response: {}", e);
            EvaluationError::InvalidProviderResponse
        })?;
        Ok(Self { response, raw })
    }

    /// Resolve an AgentAssertion variant to a JSON value for the shared AssertionEvaluator.
    pub fn build_context(&self, assertion: &AgentAssertion) -> Result<Value, EvaluationError> {
        match assertion {
            AgentAssertion::ToolCalled { name } => {
                let found = self
                    .response
                    .get_tool_calls()
                    .iter()
                    .any(|tc| tc.name == *name);
                Ok(json!(found))
            }
            AgentAssertion::ToolNotCalled { name } => {
                let not_found = !self
                    .response
                    .get_tool_calls()
                    .iter()
                    .any(|tc| tc.name == *name);
                Ok(json!(not_found))
            }
            AgentAssertion::ToolCalledWithArgs { name, arguments } => {
                let matched =
                    self.response.get_tool_calls().iter().any(|tc| {
                        tc.name == *name && Self::partial_match(&tc.arguments, &arguments.0)
                    });
                Ok(json!(matched))
            }
            AgentAssertion::ToolCallSequence { names } => {
                let actual: Vec<String> = self
                    .response
                    .get_tool_calls()
                    .iter()
                    .map(|tc| tc.name.clone())
                    .collect();
                let mut expected_iter = names.iter();
                let mut current = expected_iter.next();
                for actual_name in &actual {
                    if let Some(exp) = current {
                        if actual_name == exp {
                            current = expected_iter.next();
                        }
                    }
                }
                Ok(json!(current.is_none()))
            }
            AgentAssertion::ToolCallCount { name } => {
                let tools = &self.response.get_tool_calls();
                let count = if let Some(name) = name {
                    tools.iter().filter(|tc| tc.name == *name).count()
                } else {
                    tools.len()
                };
                Ok(json!(count))
            }
            AgentAssertion::ToolArgument { name, argument_key } => {
                let value = self
                    .response
                    .get_tool_calls()
                    .iter()
                    .find(|tc| tc.name == *name)
                    .and_then(|tc| tc.arguments.get(argument_key))
                    .cloned()
                    .unwrap_or(Value::Null);

                Ok(value)
            }
            AgentAssertion::ToolResult { name } => {
                let value = self
                    .response
                    .get_tool_calls()
                    .iter()
                    .find(|tc| tc.name == *name)
                    .and_then(|tc| tc.result.clone())
                    .unwrap_or(Value::Null);

                Ok(value)
            }
            AgentAssertion::ResponseContent {} => {
                let text = self.response.response_text();
                if text.is_empty() {
                    Ok(Value::Null)
                } else {
                    Ok(json!(text))
                }
            }
            AgentAssertion::ResponseModel {} => Ok(self
                .response
                .model_name()
                .map(|m| json!(m))
                .unwrap_or(Value::Null)),
            AgentAssertion::ResponseFinishReason {} => Ok(self
                .response
                .finish_reason_str()
                .map(|f| json!(f))
                .unwrap_or(Value::Null)),
            AgentAssertion::ResponseInputTokens {} => Ok(self
                .response
                .input_tokens()
                .map(|t| json!(t))
                .unwrap_or(Value::Null)),
            AgentAssertion::ResponseOutputTokens {} => Ok(self
                .response
                .output_tokens()
                .map(|t| json!(t))
                .unwrap_or(Value::Null)),
            AgentAssertion::ResponseTotalTokens {} => Ok(self
                .response
                .total_tokens()
                .map(|t| json!(t))
                .unwrap_or(Value::Null)),
            AgentAssertion::ResponseField { path } => Self::extract_by_path(self.raw, path),
        }
    }

    // ─── Helpers ───────────────────────────────────────────────────────

    /// Check if all specified args are present and equal in actual args (partial match).
    fn partial_match(actual: &Value, expected: &Value) -> bool {
        match (actual, expected) {
            (Value::Object(actual_map), Value::Object(expected_map)) => {
                for (key, expected_val) in expected_map {
                    match actual_map.get(key) {
                        Some(actual_val) => {
                            if !Self::partial_match(actual_val, expected_val) {
                                return false;
                            }
                        }
                        None => return false,
                    }
                }
                true
            }
            _ => actual == expected,
        }
    }

    fn extract_by_path(val: &Value, path: &str) -> Result<Value, EvaluationError> {
        FieldEvaluator::extract_field_value(val, path).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scouter_types::agent::PyValueWrapper;

    #[test]
    fn test_tool_called_assertion() {
        let context = json!({
            "model": "gpt-4o",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {"name": "web_search", "arguments": "{\"query\": \"test\"}"}
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });

        let builder = AgentContextBuilder::from_context(&context, None, None).unwrap();

        let result = builder
            .build_context(&AgentAssertion::ToolCalled {
                name: "web_search".to_string(),
            })
            .unwrap();
        assert_eq!(result, json!(true));

        let result = builder
            .build_context(&AgentAssertion::ToolNotCalled {
                name: "delete_user".to_string(),
            })
            .unwrap();
        assert_eq!(result, json!(true));

        let result = builder
            .build_context(&AgentAssertion::ToolCallCount { name: None })
            .unwrap();
        assert_eq!(result, json!(1));
    }

    #[test]
    fn test_tool_called_with_args_partial_match() {
        let context = json!({
            "model": "gpt-4o",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {"name": "web_search", "arguments": "{\"query\": \"weather NYC\", \"lang\": \"en\", \"limit\": 5}"}
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });

        let builder = AgentContextBuilder::from_context(&context, None, None).unwrap();

        // Partial match - only checking "query"
        let result = builder
            .build_context(&AgentAssertion::ToolCalledWithArgs {
                name: "web_search".to_string(),
                arguments: PyValueWrapper(json!({"query": "weather NYC"})),
            })
            .unwrap();
        assert_eq!(result, json!(true));

        // Non-matching arg
        let result = builder
            .build_context(&AgentAssertion::ToolCalledWithArgs {
                name: "web_search".to_string(),
                arguments: PyValueWrapper(json!({"query": "weather LA"})),
            })
            .unwrap();
        assert_eq!(result, json!(false));
    }

    #[test]
    fn test_tool_call_sequence() {
        let context = json!({
            "model": "gpt-4o",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [
                        {"id": "call_1", "type": "function", "function": {"name": "web_search", "arguments": "{}"}},
                        {"id": "call_2", "type": "function", "function": {"name": "summarize", "arguments": "{}"}},
                        {"id": "call_3", "type": "function", "function": {"name": "respond", "arguments": "{}"}}
                    ]
                },
                "finish_reason": "tool_calls"
            }]
        });

        let builder = AgentContextBuilder::from_context(&context, None, None).unwrap();

        let result = builder
            .build_context(&AgentAssertion::ToolCallSequence {
                names: vec![
                    "web_search".to_string(),
                    "summarize".to_string(),
                    "respond".to_string(),
                ],
            })
            .unwrap();
        assert_eq!(result, json!(true));

        // Wrong order
        let result = builder
            .build_context(&AgentAssertion::ToolCallSequence {
                names: vec!["respond".to_string(), "web_search".to_string()],
            })
            .unwrap();
        assert_eq!(result, json!(false));
    }

    #[test]
    fn test_response_field_escape_hatch() {
        let context = json!({
            "response": {
                "candidates": [{
                    "content": {"role": "model", "parts": [{"text": "hello"}]},
                    "finishReason": "STOP",
                    "safety_ratings": [{"category": "HARM_CATEGORY_SAFE"}]
                }],
                "usageMetadata": {"promptTokenCount": 5, "candidatesTokenCount": 2}
            }
        });

        let builder = AgentContextBuilder::from_context(&context, None, Some("response")).unwrap();

        let result = builder
            .build_context(&AgentAssertion::ResponseField {
                path: "candidates[0].safety_ratings[0].category".to_string(),
            })
            .unwrap();
        assert_eq!(result, json!("HARM_CATEGORY_SAFE"));
    }

    #[test]
    fn test_no_tool_calls() {
        let context = json!({
            "model": "gpt-4o",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Just a text response."
                },
                "finish_reason": "stop"
            }]
        });

        let builder = AgentContextBuilder::from_context(&context, None, None).unwrap();

        let result = builder
            .build_context(&AgentAssertion::ToolNotCalled {
                name: "web_search".to_string(),
            })
            .unwrap();
        assert_eq!(result, json!(true));
    }

    #[test]
    fn test_from_context_invalid_json() {
        // Empty object has no recognizable vendor keys
        let context = json!({});
        let result = AgentContextBuilder::from_context(&context, None, None);
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(EvaluationError::InvalidProviderResponse)
        ));
    }

    #[test]
    fn test_tool_call_sequence_subsequence() {
        let context = json!({
            "model": "gpt-4o",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [
                        {"id": "c1", "type": "function", "function": {"name": "search", "arguments": "{}"}},
                        {"id": "c2", "type": "function", "function": {"name": "filter", "arguments": "{}"}},
                        {"id": "c3", "type": "function", "function": {"name": "rank", "arguments": "{}"}},
                        {"id": "c4", "type": "function", "function": {"name": "respond", "arguments": "{}"}}
                    ]
                },
                "finish_reason": "tool_calls"
            }]
        });

        let builder = AgentContextBuilder::from_context(&context, None, None).unwrap();

        // Non-contiguous in-order subsequence should pass
        let result = builder
            .build_context(&AgentAssertion::ToolCallSequence {
                names: vec![
                    "search".to_string(),
                    "rank".to_string(),
                    "respond".to_string(),
                ],
            })
            .unwrap();
        assert_eq!(result, json!(true));

        // Out-of-order should fail
        let result = builder
            .build_context(&AgentAssertion::ToolCallSequence {
                names: vec!["respond".to_string(), "search".to_string()],
            })
            .unwrap();
        assert_eq!(result, json!(false));
    }

    #[test]
    fn test_path_extraction_errors() {
        use crate::tasks::evaluator::{FieldEvaluator, MAX_PATH_LEN, MAX_PATH_SEGMENTS};

        let context = json!({});

        let result = FieldEvaluator::extract_field_value(&context, "");
        assert!(matches!(result, Err(EvaluationError::EmptyFieldPath)));

        let long_path = "a".repeat(MAX_PATH_LEN + 1);
        let result = FieldEvaluator::extract_field_value(&context, &long_path);
        assert!(matches!(result, Err(EvaluationError::PathTooLong(_))));

        let many_segments = (0..MAX_PATH_SEGMENTS + 1)
            .map(|i| format!("seg{}", i))
            .collect::<Vec<_>>()
            .join(".");
        let result = FieldEvaluator::extract_field_value(&context, &many_segments);
        assert!(matches!(
            result,
            Err(EvaluationError::TooManyPathSegments(_))
        ));
    }

    #[test]
    fn test_response_content_empty() {
        let context = json!({
            "model": "gpt-4o",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null
                },
                "finish_reason": "stop"
            }]
        });

        let builder = AgentContextBuilder::from_context(&context, None, None).unwrap();
        let result = builder
            .build_context(&AgentAssertion::ResponseContent {})
            .unwrap();
        assert_eq!(result, Value::Null);
    }

    #[test]
    fn test_partial_match_nested() {
        let context = json!({
            "model": "gpt-4o",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "c1",
                        "type": "function",
                        "function": {"name": "create_item", "arguments": "{\"item\": {\"name\": \"widget\", \"price\": 9.99, \"tags\": [\"sale\"]}}"}
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });

        let builder = AgentContextBuilder::from_context(&context, None, None).unwrap();

        // Nested partial match - only check inner "name"
        let result = builder
            .build_context(&AgentAssertion::ToolCalledWithArgs {
                name: "create_item".to_string(),
                arguments: PyValueWrapper(json!({"item": {"name": "widget"}})),
            })
            .unwrap();
        assert_eq!(result, json!(true));

        // Nested mismatch
        let result = builder
            .build_context(&AgentAssertion::ToolCalledWithArgs {
                name: "create_item".to_string(),
                arguments: PyValueWrapper(json!({"item": {"name": "gadget"}})),
            })
            .unwrap();
        assert_eq!(result, json!(false));
    }

    #[test]
    fn test_tool_result_extraction() {
        // tool result values are not standard in OpenAI format; test ToolResult returns Null
        // when no result is present (tool_calls don't carry result in request JSON)
        let context = json!({
            "model": "gpt-4o",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "c1",
                        "type": "function",
                        "function": {"name": "web_search", "arguments": "{\"query\": \"test\"}"}
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });

        let builder = AgentContextBuilder::from_context(&context, None, None).unwrap();

        // Named tool with no result -> Null
        let result = builder
            .build_context(&AgentAssertion::ToolResult {
                name: "web_search".to_string(),
            })
            .unwrap();
        assert_eq!(result, Value::Null);

        // Missing tool name -> Null
        let result = builder
            .build_context(&AgentAssertion::ToolResult {
                name: "nonexistent".to_string(),
            })
            .unwrap();
        assert_eq!(result, Value::Null);
    }

    #[test]
    fn test_tool_argument_extraction() {
        let context = json!({
            "model": "gpt-4o",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {"name": "web_search", "arguments": "{\"query\": \"test query\", \"limit\": 10}"}
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });

        let builder = AgentContextBuilder::from_context(&context, None, None).unwrap();

        let result = builder
            .build_context(&AgentAssertion::ToolArgument {
                name: "web_search".to_string(),
                argument_key: "query".to_string(),
            })
            .unwrap();
        assert_eq!(result, json!("test query"));

        let result = builder
            .build_context(&AgentAssertion::ToolArgument {
                name: "web_search".to_string(),
                argument_key: "missing".to_string(),
            })
            .unwrap();
        assert_eq!(result, Value::Null);
    }
}
