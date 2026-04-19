use super::{Attribute, SpanId, TraceId, TraceSpanRecord};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// Attribute key constants
pub const GEN_AI_OPERATION_NAME: &str = "gen_ai.operation.name";
pub const GEN_AI_PROVIDER_NAME: &str = "gen_ai.provider.name";
pub const GEN_AI_REQUEST_MODEL: &str = "gen_ai.request.model";
pub const GEN_AI_RESPONSE_MODEL: &str = "gen_ai.response.model";
pub const GEN_AI_RESPONSE_ID: &str = "gen_ai.response.id";
pub const GEN_AI_USAGE_INPUT_TOKENS: &str = "gen_ai.usage.input_tokens";
pub const GEN_AI_USAGE_OUTPUT_TOKENS: &str = "gen_ai.usage.output_tokens";
pub const GEN_AI_USAGE_CACHE_CREATION_INPUT_TOKENS: &str =
    "gen_ai.usage.cache_creation.input_tokens";
pub const GEN_AI_USAGE_CACHE_READ_INPUT_TOKENS: &str = "gen_ai.usage.cache_read.input_tokens";
pub const GEN_AI_RESPONSE_FINISH_REASONS: &str = "gen_ai.response.finish_reasons";
pub const GEN_AI_OUTPUT_TYPE: &str = "gen_ai.output.type";
pub const GEN_AI_CONVERSATION_ID: &str = "gen_ai.conversation.id";
pub const GEN_AI_AGENT_NAME: &str = "gen_ai.agent.name";
pub const GEN_AI_AGENT_ID: &str = "gen_ai.agent.id";
pub const GEN_AI_TOOL_NAME: &str = "gen_ai.tool.name";
pub const GEN_AI_TOOL_TYPE: &str = "gen_ai.tool.type";
pub const GEN_AI_TOOL_CALL_ID: &str = "gen_ai.tool.call.id";
pub const GEN_AI_REQUEST_TEMPERATURE: &str = "gen_ai.request.temperature";
pub const GEN_AI_REQUEST_MAX_TOKENS: &str = "gen_ai.request.max_tokens";
pub const GEN_AI_REQUEST_TOP_P: &str = "gen_ai.request.top_p";
pub const GEN_AI_ERROR_TYPE: &str = "error.type";
pub const OPENAI_API_TYPE: &str = "openai.api.type";
pub const OPENAI_SERVICE_TIER: &str = "openai.response.service_tier";

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct GenAiSpanRecord {
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub service_name: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration_ms: i64,
    pub status_code: i32,
    pub operation_name: Option<String>,
    pub provider_name: Option<String>,
    pub request_model: Option<String>,
    pub response_model: Option<String>,
    pub response_id: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_creation_input_tokens: Option<i64>,
    pub cache_read_input_tokens: Option<i64>,
    pub finish_reasons: Vec<String>,
    pub output_type: Option<String>,
    pub conversation_id: Option<String>,
    pub agent_name: Option<String>,
    pub agent_id: Option<String>,
    pub tool_name: Option<String>,
    pub tool_type: Option<String>,
    pub tool_call_id: Option<String>,
    pub request_temperature: Option<f64>,
    pub request_max_tokens: Option<i64>,
    pub request_top_p: Option<f64>,
    pub error_type: Option<String>,
    pub openai_api_type: Option<String>,
    pub openai_service_tier: Option<String>,
    pub label: Option<String>,
}

fn attr_as_i64(v: &serde_json::Value) -> Option<i64> {
    match v {
        serde_json::Value::Number(n) => n.as_i64().or_else(|| {
            n.as_f64().and_then(|f| {
                if f.is_finite() && f >= i64::MIN as f64 && f <= i64::MAX as f64 {
                    Some(f as i64)
                } else {
                    None
                }
            })
        }),
        serde_json::Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

fn attr_as_f64(v: &serde_json::Value) -> Option<f64> {
    match v {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

fn parse_finish_reasons(v: &serde_json::Value) -> Vec<String> {
    match v {
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|item| {
                if let serde_json::Value::String(s) = item {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .collect(),
        serde_json::Value::String(s) => {
            if let Ok(serde_json::Value::Array(arr)) = serde_json::from_str(s) {
                arr.into_iter()
                    .filter_map(|item| {
                        if let serde_json::Value::String(s) = item {
                            Some(s)
                        } else {
                            None
                        }
                    })
                    .collect()
            } else {
                vec![s.clone()]
            }
        }
        _ => vec![],
    }
}

/// Returns None if gen_ai.operation.name is not present in span attributes.
/// Scans attributes once, extracting all gen_ai.* fields.
pub fn extract_gen_ai_span(record: &TraceSpanRecord) -> Option<GenAiSpanRecord> {
    let mut out = GenAiSpanRecord {
        trace_id: record.trace_id,
        span_id: record.span_id.clone(),
        service_name: record.service_name.clone(),
        start_time: record.start_time,
        end_time: Some(record.end_time),
        duration_ms: record.duration_ms,
        status_code: record.status_code,
        label: record.label.clone(),
        ..Default::default()
    };

    for Attribute { key, value } in &record.attributes {
        match key.as_str() {
            GEN_AI_OPERATION_NAME => {
                if let serde_json::Value::String(s) = value {
                    out.operation_name = Some(s.clone());
                }
            }
            GEN_AI_PROVIDER_NAME => {
                if let serde_json::Value::String(s) = value {
                    out.provider_name = Some(s.clone());
                }
            }
            GEN_AI_REQUEST_MODEL => {
                if let serde_json::Value::String(s) = value {
                    out.request_model = Some(s.clone());
                }
            }
            GEN_AI_RESPONSE_MODEL => {
                if let serde_json::Value::String(s) = value {
                    out.response_model = Some(s.clone());
                }
            }
            GEN_AI_RESPONSE_ID => {
                if let serde_json::Value::String(s) = value {
                    out.response_id = Some(s.clone());
                }
            }
            GEN_AI_USAGE_INPUT_TOKENS => {
                out.input_tokens = attr_as_i64(value);
            }
            GEN_AI_USAGE_OUTPUT_TOKENS => {
                out.output_tokens = attr_as_i64(value);
            }
            GEN_AI_USAGE_CACHE_CREATION_INPUT_TOKENS => {
                out.cache_creation_input_tokens = attr_as_i64(value);
            }
            GEN_AI_USAGE_CACHE_READ_INPUT_TOKENS => {
                out.cache_read_input_tokens = attr_as_i64(value);
            }
            GEN_AI_RESPONSE_FINISH_REASONS => {
                out.finish_reasons = parse_finish_reasons(value);
            }
            GEN_AI_OUTPUT_TYPE => {
                if let serde_json::Value::String(s) = value {
                    out.output_type = Some(s.clone());
                }
            }
            GEN_AI_CONVERSATION_ID => {
                if let serde_json::Value::String(s) = value {
                    out.conversation_id = Some(s.clone());
                }
            }
            GEN_AI_AGENT_NAME => {
                if let serde_json::Value::String(s) = value {
                    out.agent_name = Some(s.clone());
                }
            }
            GEN_AI_AGENT_ID => {
                if let serde_json::Value::String(s) = value {
                    out.agent_id = Some(s.clone());
                }
            }
            GEN_AI_TOOL_NAME => {
                if let serde_json::Value::String(s) = value {
                    out.tool_name = Some(s.clone());
                }
            }
            GEN_AI_TOOL_TYPE => {
                if let serde_json::Value::String(s) = value {
                    out.tool_type = Some(s.clone());
                }
            }
            GEN_AI_TOOL_CALL_ID => {
                if let serde_json::Value::String(s) = value {
                    out.tool_call_id = Some(s.clone());
                }
            }
            GEN_AI_REQUEST_TEMPERATURE => {
                out.request_temperature = attr_as_f64(value);
            }
            GEN_AI_REQUEST_MAX_TOKENS => {
                out.request_max_tokens = attr_as_i64(value);
            }
            GEN_AI_REQUEST_TOP_P => {
                out.request_top_p = attr_as_f64(value);
            }
            GEN_AI_ERROR_TYPE => {
                if let serde_json::Value::String(s) = value {
                    out.error_type = Some(s.clone());
                }
            }
            OPENAI_API_TYPE => {
                if let serde_json::Value::String(s) = value {
                    out.openai_api_type = Some(s.clone());
                }
            }
            OPENAI_SERVICE_TIER => {
                if let serde_json::Value::String(s) = value {
                    out.openai_service_tier = Some(s.clone());
                }
            }
            _ => {}
        }
    }

    out.operation_name.as_ref()?;

    Some(out)
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct GenAiTokenBucket {
    pub bucket_start: DateTime<Utc>,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cache_creation_tokens: i64,
    pub total_cache_read_tokens: i64,
    pub span_count: i64,
    pub error_rate: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct GenAiOperationBreakdown {
    pub operation_name: String,
    pub provider_name: Option<String>,
    pub span_count: i64,
    pub avg_duration_ms: f64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub error_rate: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct GenAiModelUsage {
    pub model: String,
    pub provider_name: Option<String>,
    pub span_count: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub p50_duration_ms: Option<f64>,
    pub p95_duration_ms: Option<f64>,
    pub error_rate: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct GenAiAgentActivity {
    pub agent_name: Option<String>,
    pub agent_id: Option<String>,
    pub conversation_id: Option<String>,
    pub span_count: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub last_seen: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct GenAiToolActivity {
    pub tool_name: Option<String>,
    pub tool_type: Option<String>,
    pub call_count: i64,
    pub avg_duration_ms: f64,
    pub error_rate: f64,
}

fn default_bucket_interval() -> String {
    "hour".to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenAiMetricsRequest {
    pub service_name: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    #[serde(default = "default_bucket_interval")]
    pub bucket_interval: String,
    pub operation_name: Option<String>,
    pub provider_name: Option<String>,
    pub model: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct GenAiSpanFilters {
    pub service_name: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub operation_name: Option<String>,
    pub provider_name: Option<String>,
    pub model: Option<String>,
    pub conversation_id: Option<String>,
    pub agent_name: Option<String>,
    pub tool_name: Option<String>,
    pub error_type: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct GenAiErrorCount {
    pub error_type: String,
    pub count: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct GenAiTokenMetricsResponse {
    pub buckets: Vec<GenAiTokenBucket>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct GenAiOperationBreakdownResponse {
    pub operations: Vec<GenAiOperationBreakdown>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct GenAiModelUsageResponse {
    pub models: Vec<GenAiModelUsage>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct GenAiAgentActivityResponse {
    pub agents: Vec<GenAiAgentActivity>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct GenAiToolActivityResponse {
    pub tools: Vec<GenAiToolActivity>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct GenAiErrorBreakdownResponse {
    pub errors: Vec<GenAiErrorCount>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct GenAiSpansResponse {
    pub spans: Vec<GenAiSpanRecord>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::{SpanId, TraceId};

    fn make_span(attrs: Vec<(&str, serde_json::Value)>) -> TraceSpanRecord {
        let attributes = attrs
            .into_iter()
            .map(|(k, v)| Attribute {
                key: k.to_string(),
                value: v,
            })
            .collect();
        TraceSpanRecord {
            trace_id: TraceId::from_bytes([0u8; 16]),
            span_id: SpanId::from_bytes([1u8; 8]),
            service_name: "test-service".to_string(),
            start_time: DateTime::from_timestamp_millis(1_000_000).unwrap(),
            end_time: DateTime::from_timestamp_millis(1_001_000).unwrap(),
            duration_ms: 1000,
            status_code: 0,
            attributes,
            ..Default::default()
        }
    }

    #[test]
    fn test_extract_gen_ai_span_returns_none_without_operation() {
        let span = make_span(vec![]);
        assert!(extract_gen_ai_span(&span).is_none());
    }

    #[test]
    fn test_extract_gen_ai_span_returns_none_missing_operation_name() {
        let span = make_span(vec![(
            GEN_AI_USAGE_INPUT_TOKENS,
            serde_json::Value::Number(100.into()),
        )]);
        assert!(extract_gen_ai_span(&span).is_none());
    }

    #[test]
    fn test_extract_gen_ai_span_basic() {
        let span = make_span(vec![
            (
                GEN_AI_OPERATION_NAME,
                serde_json::Value::String("chat".to_string()),
            ),
            (
                GEN_AI_PROVIDER_NAME,
                serde_json::Value::String("anthropic".to_string()),
            ),
            (
                GEN_AI_USAGE_INPUT_TOKENS,
                serde_json::Value::Number(100.into()),
            ),
            (
                GEN_AI_USAGE_OUTPUT_TOKENS,
                serde_json::Value::Number(200.into()),
            ),
        ]);

        let result = extract_gen_ai_span(&span).expect("should extract");
        assert_eq!(result.operation_name.as_deref(), Some("chat"));
        assert_eq!(result.provider_name.as_deref(), Some("anthropic"));
        assert_eq!(result.input_tokens, Some(100));
        assert_eq!(result.output_tokens, Some(200));
        assert_eq!(result.service_name, "test-service");
        assert_eq!(result.duration_ms, 1000);
    }

    #[test]
    fn test_extract_gen_ai_span_finish_reasons_array() {
        let span = make_span(vec![
            (
                GEN_AI_OPERATION_NAME,
                serde_json::Value::String("chat".to_string()),
            ),
            (
                GEN_AI_RESPONSE_FINISH_REASONS,
                serde_json::Value::Array(vec![serde_json::Value::String("stop".to_string())]),
            ),
        ]);

        let result = extract_gen_ai_span(&span).expect("should extract");
        assert_eq!(result.finish_reasons, vec!["stop"]);
    }

    #[test]
    fn test_extract_gen_ai_span_finish_reasons_string() {
        let span = make_span(vec![
            (
                GEN_AI_OPERATION_NAME,
                serde_json::Value::String("chat".to_string()),
            ),
            (
                GEN_AI_RESPONSE_FINISH_REASONS,
                serde_json::Value::String("[\"stop\",\"length\"]".to_string()),
            ),
        ]);

        let result = extract_gen_ai_span(&span).expect("should extract");
        assert_eq!(result.finish_reasons, vec!["stop", "length"]);
    }

    #[test]
    fn test_attr_as_i64_variants() {
        assert_eq!(
            attr_as_i64(&serde_json::Value::Number(42.into())),
            Some(42)
        );
        assert_eq!(
            attr_as_i64(&serde_json::Value::String("100".to_string())),
            Some(100)
        );
        assert_eq!(attr_as_i64(&serde_json::Value::Null), None);
    }

    #[test]
    fn test_attr_as_i64_float_encoded() {
        assert_eq!(attr_as_i64(&serde_json::json!(100.0)), Some(100));
        assert_eq!(attr_as_i64(&serde_json::json!(0.0)), Some(0));
    }

    #[test]
    fn test_parse_finish_reasons_plain_string() {
        let result = parse_finish_reasons(&serde_json::Value::String("stop".to_string()));
        assert_eq!(result, vec!["stop".to_string()]);
    }

    #[test]
    fn test_parse_finish_reasons_mixed_array() {
        let v = serde_json::json!(["stop", 1, null, "length"]);
        let result = parse_finish_reasons(&v);
        assert_eq!(result, vec!["stop".to_string(), "length".to_string()]);
    }

    #[test]
    fn test_attr_as_f64_variants() {
        assert_eq!(
            attr_as_f64(&serde_json::Value::Number(
                serde_json::Number::from_f64(0.7).unwrap()
            )),
            Some(0.7)
        );
        assert_eq!(
            attr_as_f64(&serde_json::Value::String("0.9".to_string())),
            Some(0.9)
        );
        assert_eq!(attr_as_f64(&serde_json::Value::Null), None);
    }

    #[test]
    fn test_extract_gen_ai_span_agent_fields() {
        let span = make_span(vec![
            (
                GEN_AI_OPERATION_NAME,
                serde_json::Value::String("chat".to_string()),
            ),
            (
                GEN_AI_AGENT_NAME,
                serde_json::Value::String("my-agent".to_string()),
            ),
            (
                GEN_AI_AGENT_ID,
                serde_json::Value::String("agent-123".to_string()),
            ),
            (
                GEN_AI_CONVERSATION_ID,
                serde_json::Value::String("conv-456".to_string()),
            ),
        ]);

        let result = extract_gen_ai_span(&span).expect("should extract");
        assert_eq!(result.agent_name.as_deref(), Some("my-agent"));
        assert_eq!(result.agent_id.as_deref(), Some("agent-123"));
        assert_eq!(result.conversation_id.as_deref(), Some("conv-456"));
    }

    #[test]
    fn test_extract_gen_ai_span_tool_fields() {
        let span = make_span(vec![
            (
                GEN_AI_OPERATION_NAME,
                serde_json::Value::String("execute_tool".to_string()),
            ),
            (
                GEN_AI_TOOL_NAME,
                serde_json::Value::String("web_search".to_string()),
            ),
            (
                GEN_AI_TOOL_TYPE,
                serde_json::Value::String("function".to_string()),
            ),
            (
                GEN_AI_TOOL_CALL_ID,
                serde_json::Value::String("call-789".to_string()),
            ),
        ]);

        let result = extract_gen_ai_span(&span).expect("should extract");
        assert_eq!(result.tool_name.as_deref(), Some("web_search"));
        assert_eq!(result.tool_type.as_deref(), Some("function"));
        assert_eq!(result.tool_call_id.as_deref(), Some("call-789"));
    }
}
