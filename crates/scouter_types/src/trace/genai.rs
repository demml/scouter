use super::{Attribute, SpanId, TraceId, TraceSpanRecord};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── Span attribute key constants ─────────────────────────────────────────────
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

// Missing span attribute constants (agent, data source, request params, server)
pub const GEN_AI_AGENT_DESCRIPTION: &str = "gen_ai.agent.description";
pub const GEN_AI_AGENT_VERSION: &str = "gen_ai.agent.version";
pub const GEN_AI_DATA_SOURCE_ID: &str = "gen_ai.data_source.id";
pub const GEN_AI_REQUEST_CHOICE_COUNT: &str = "gen_ai.request.choice.count";
pub const GEN_AI_REQUEST_SEED: &str = "gen_ai.request.seed";
pub const GEN_AI_REQUEST_FREQUENCY_PENALTY: &str = "gen_ai.request.frequency_penalty";
pub const GEN_AI_REQUEST_PRESENCE_PENALTY: &str = "gen_ai.request.presence_penalty";
pub const GEN_AI_REQUEST_STOP_SEQUENCES: &str = "gen_ai.request.stop_sequences";
pub const SERVER_ADDRESS: &str = "server.address";
pub const SERVER_PORT: &str = "server.port";

// Opt-in content attributes — can appear on spans (JSON string) or events (structured)
pub const GEN_AI_INPUT_MESSAGES: &str = "gen_ai.input.messages";
pub const GEN_AI_OUTPUT_MESSAGES: &str = "gen_ai.output.messages";
pub const GEN_AI_SYSTEM_INSTRUCTIONS: &str = "gen_ai.system_instructions";
pub const GEN_AI_TOOL_DEFINITIONS: &str = "gen_ai.tool.definitions";

// ── OTel event names ──────────────────────────────────────────────────────────
// Source: https://opentelemetry.io/docs/specs/semconv/gen-ai/gen-ai-events/
//
// gen_ai.client.inference.operation.details:
//   Carries the same scalar attrs as the parent span PLUS opt-in content (messages,
//   system instructions, tool defs). Designed for standalone use (no span context
//   required). When attached to a span, scalars are redundant — span attrs win.
//   Only the 4 opt-in content fields use event fallback; all other scalars come
//   from span attributes only.
//
// gen_ai.evaluation.result:
//   Evaluation scores/labels attached to a span. No span attribute equivalent.
//   Always extracted from events.
pub const GEN_AI_EVENT_INFERENCE_DETAILS: &str = "gen_ai.client.inference.operation.details";
pub const GEN_AI_EVENT_EVALUATION_RESULT: &str = "gen_ai.evaluation.result";

// Eval event attribute constants
pub const GEN_AI_EVALUATION_NAME: &str = "gen_ai.evaluation.name";
pub const GEN_AI_EVALUATION_SCORE_LABEL: &str = "gen_ai.evaluation.score.label";
pub const GEN_AI_EVALUATION_SCORE_VALUE: &str = "gen_ai.evaluation.score.value";
pub const GEN_AI_EVALUATION_EXPLANATION: &str = "gen_ai.evaluation.explanation";

// ── GenAiEvalResult ───────────────────────────────────────────────────────────

/// Result from a gen_ai.evaluation.result event attached to a span.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiEvalResult {
    pub name: String,
    pub score_label: Option<String>,
    pub score_value: Option<f64>,
    pub explanation: Option<String>,
    pub response_id: Option<String>,
}

// ── GenAiSpanRecord ───────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiSpanRecord {
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub service_name: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration_ms: i64,
    pub status_code: i32,
    // Core operation scalars (from span attributes)
    pub operation_name: Option<String>,
    pub provider_name: Option<String>,
    pub request_model: Option<String>,
    pub response_model: Option<String>,
    pub response_id: Option<String>,
    // Token usage (from span attributes)
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cache_creation_input_tokens: Option<i64>,
    pub cache_read_input_tokens: Option<i64>,
    pub finish_reasons: Vec<String>,
    pub output_type: Option<String>,
    // Conversation / agent (from span attributes)
    pub conversation_id: Option<String>,
    pub agent_name: Option<String>,
    pub agent_id: Option<String>,
    pub agent_description: Option<String>,
    pub agent_version: Option<String>,
    pub data_source_id: Option<String>,
    // Tool (from span attributes)
    pub tool_name: Option<String>,
    pub tool_type: Option<String>,
    pub tool_call_id: Option<String>,
    // Request parameters (from span attributes)
    pub request_temperature: Option<f64>,
    pub request_max_tokens: Option<i64>,
    pub request_top_p: Option<f64>,
    pub request_choice_count: Option<i64>,
    pub request_seed: Option<i64>,
    pub request_frequency_penalty: Option<f64>,
    pub request_presence_penalty: Option<f64>,
    pub request_stop_sequences: Vec<String>,
    // Server info (from span attributes)
    pub server_address: Option<String>,
    pub server_port: Option<i64>,
    // Error / provider-specific (from span attributes)
    pub error_type: Option<String>,
    pub openai_api_type: Option<String>,
    pub openai_service_tier: Option<String>,
    pub label: Option<String>,
    // Opt-in content: span attr first, gen_ai.client.inference.operation.details event fallback.
    // Stored as JSON string (spec allows JSON-serialized string on spans; events carry structured form).
    pub input_messages: Option<String>,
    pub output_messages: Option<String>,
    pub system_instructions: Option<String>,
    pub tool_definitions: Option<String>,
    // Evaluation results from gen_ai.evaluation.result events (always from events, no span equivalent).
    pub eval_results: Vec<GenAiEvalResult>,
}

// ── Helper functions ──────────────────────────────────────────────────────────

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

fn parse_stop_sequences(v: &serde_json::Value) -> Vec<String> {
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
                vec![]
            }
        }
        _ => vec![],
    }
}

/// Serialize an attribute value to a JSON string.
/// String values are returned as-is (no extra quoting).
/// Structured values (arrays, objects) are serialized to compact JSON.
fn value_to_json_string(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Null => None,
        other => serde_json::to_string(other).ok(),
    }
}

// ── Event extraction ──────────────────────────────────────────────────────────

/// Intermediate struct returned by extract_gen_ai_events.
pub struct GenAiEventData {
    pub input_messages: Option<String>,
    pub output_messages: Option<String>,
    pub system_instructions: Option<String>,
    pub tool_definitions: Option<String>,
    pub eval_results: Vec<GenAiEvalResult>,
}

/// Extract data from OTel events attached to a span.
///
/// Two event types are handled:
///
/// - `gen_ai.client.inference.operation.details`: carries opt-in content attributes
///   (gen_ai.input.messages, gen_ai.output.messages, gen_ai.system_instructions,
///   gen_ai.tool.definitions). Used as fallback when the same attrs are absent on
///   the parent span. Scalar attrs on this event are ignored — span attrs are
///   authoritative for all scalar data (tokens, model, operation, etc.).
///
/// - `gen_ai.evaluation.result`: carries evaluation scores. Always extracted from
///   events — there is no span attribute equivalent.
pub fn extract_gen_ai_events(record: &TraceSpanRecord) -> GenAiEventData {
    let mut input_messages: Option<String> = None;
    let mut output_messages: Option<String> = None;
    let mut system_instructions: Option<String> = None;
    let mut tool_definitions: Option<String> = None;
    let mut eval_results: Vec<GenAiEvalResult> = Vec::new();

    for event in &record.events {
        match event.name.as_str() {
            GEN_AI_EVENT_INFERENCE_DETAILS => {
                for Attribute { key, value } in &event.attributes {
                    match key.as_str() {
                        GEN_AI_INPUT_MESSAGES => {
                            input_messages = value_to_json_string(value);
                        }
                        GEN_AI_OUTPUT_MESSAGES => {
                            output_messages = value_to_json_string(value);
                        }
                        GEN_AI_SYSTEM_INSTRUCTIONS => {
                            system_instructions = value_to_json_string(value);
                        }
                        GEN_AI_TOOL_DEFINITIONS => {
                            tool_definitions = value_to_json_string(value);
                        }
                        _ => {}
                    }
                }
            }
            GEN_AI_EVENT_EVALUATION_RESULT => {
                let mut result = GenAiEvalResult::default();
                for Attribute { key, value } in &event.attributes {
                    match key.as_str() {
                        GEN_AI_EVALUATION_NAME => {
                            if let serde_json::Value::String(s) = value {
                                result.name = s.clone();
                            }
                        }
                        GEN_AI_EVALUATION_SCORE_LABEL => {
                            if let serde_json::Value::String(s) = value {
                                result.score_label = Some(s.clone());
                            }
                        }
                        GEN_AI_EVALUATION_SCORE_VALUE => {
                            result.score_value = attr_as_f64(value);
                        }
                        GEN_AI_EVALUATION_EXPLANATION => {
                            if let serde_json::Value::String(s) = value {
                                result.explanation = Some(s.clone());
                            }
                        }
                        GEN_AI_RESPONSE_ID => {
                            if let serde_json::Value::String(s) = value {
                                result.response_id = Some(s.clone());
                            }
                        }
                        _ => {}
                    }
                }
                // gen_ai.evaluation.name is required; skip malformed events
                if !result.name.is_empty() {
                    eval_results.push(result);
                }
            }
            _ => {}
        }
    }

    GenAiEventData {
        input_messages,
        output_messages,
        system_instructions,
        tool_definitions,
        eval_results,
    }
}

// ── Span extraction ───────────────────────────────────────────────────────────

/// Returns None if gen_ai.operation.name is not present in span attributes.
/// Scans span attributes once, extracting all gen_ai.* scalar fields.
/// Then merges event data: opt-in content fields use span-first / event-fallback;
/// eval_results always come from events.
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
            GEN_AI_AGENT_DESCRIPTION => {
                if let serde_json::Value::String(s) = value {
                    out.agent_description = Some(s.clone());
                }
            }
            GEN_AI_AGENT_VERSION => {
                if let serde_json::Value::String(s) = value {
                    out.agent_version = Some(s.clone());
                }
            }
            GEN_AI_DATA_SOURCE_ID => {
                if let serde_json::Value::String(s) = value {
                    out.data_source_id = Some(s.clone());
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
            GEN_AI_REQUEST_CHOICE_COUNT => {
                out.request_choice_count = attr_as_i64(value);
            }
            GEN_AI_REQUEST_SEED => {
                out.request_seed = attr_as_i64(value);
            }
            GEN_AI_REQUEST_FREQUENCY_PENALTY => {
                out.request_frequency_penalty = attr_as_f64(value);
            }
            GEN_AI_REQUEST_PRESENCE_PENALTY => {
                out.request_presence_penalty = attr_as_f64(value);
            }
            GEN_AI_REQUEST_STOP_SEQUENCES => {
                out.request_stop_sequences = parse_stop_sequences(value);
            }
            SERVER_ADDRESS => {
                if let serde_json::Value::String(s) = value {
                    out.server_address = Some(s.clone());
                }
            }
            SERVER_PORT => {
                out.server_port = attr_as_i64(value);
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
            // Opt-in content on span attributes (JSON-serialized string per spec)
            GEN_AI_INPUT_MESSAGES => {
                out.input_messages = value_to_json_string(value);
            }
            GEN_AI_OUTPUT_MESSAGES => {
                out.output_messages = value_to_json_string(value);
            }
            GEN_AI_SYSTEM_INSTRUCTIONS => {
                out.system_instructions = value_to_json_string(value);
            }
            GEN_AI_TOOL_DEFINITIONS => {
                out.tool_definitions = value_to_json_string(value);
            }
            _ => {}
        }
    }

    out.operation_name.as_ref()?;

    // Merge event data.
    // Scalars: span attrs are authoritative — no event fallback.
    // Opt-in content (4 fields): span attr wins; event is fallback if span attr absent.
    // Eval results: always from events (no span attribute equivalent).
    let event_data = extract_gen_ai_events(record);
    if out.input_messages.is_none() {
        out.input_messages = event_data.input_messages;
    }
    if out.output_messages.is_none() {
        out.output_messages = event_data.output_messages;
    }
    if out.system_instructions.is_none() {
        out.system_instructions = event_data.system_instructions;
    }
    if out.tool_definitions.is_none() {
        out.tool_definitions = event_data.tool_definitions;
    }
    out.eval_results = event_data.eval_results;

    Some(out)
}

// ── Response / aggregation types ──────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
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
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
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
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
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
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
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
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
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
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
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
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
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
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiErrorCount {
    pub error_type: String,
    pub count: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiTokenMetricsResponse {
    pub buckets: Vec<GenAiTokenBucket>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiOperationBreakdownResponse {
    pub operations: Vec<GenAiOperationBreakdown>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiModelUsageResponse {
    pub models: Vec<GenAiModelUsage>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiAgentActivityResponse {
    pub agents: Vec<GenAiAgentActivity>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiToolActivityResponse {
    pub tools: Vec<GenAiToolActivity>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiErrorBreakdownResponse {
    pub errors: Vec<GenAiErrorCount>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiSpansResponse {
    pub spans: Vec<GenAiSpanRecord>,
}

// ── Agent dashboard ───────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ModelPricing {
    pub input_per_million: f64,
    pub output_per_million: f64,
    pub cache_creation_per_million: f64,
    pub cache_read_per_million: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AgentDashboardRequest {
    pub service_name: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    #[serde(default = "default_bucket_interval")]
    pub bucket_interval: String,
    pub agent_name: Option<String>,
    pub provider_name: Option<String>,
    #[serde(default)]
    pub model_pricing: std::collections::HashMap<String, ModelPricing>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AgentMetricBucket {
    pub bucket_start: DateTime<Utc>,
    pub span_count: i64,
    pub error_count: i64,
    pub error_rate: f64,
    pub avg_duration_ms: f64,
    pub p50_duration_ms: Option<f64>,
    pub p95_duration_ms: Option<f64>,
    pub p99_duration_ms: Option<f64>,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cache_creation_tokens: i64,
    pub total_cache_read_tokens: i64,
    pub total_cost: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ModelCostBreakdown {
    pub model: String,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cache_creation_tokens: i64,
    pub total_cache_read_tokens: i64,
    pub total_cost: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AgentDashboardSummary {
    pub total_requests: i64,
    pub avg_duration_ms: f64,
    pub p50_duration_ms: Option<f64>,
    pub p95_duration_ms: Option<f64>,
    pub p99_duration_ms: Option<f64>,
    pub overall_error_rate: f64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cache_creation_tokens: i64,
    pub total_cache_read_tokens: i64,
    pub unique_agent_count: i64,
    pub unique_conversation_count: i64,
    pub cost_by_model: Vec<ModelCostBreakdown>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AgentDashboardResponse {
    pub summary: AgentDashboardSummary,
    pub buckets: Vec<AgentMetricBucket>,
}

// ── Tool dashboard ────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ToolDashboardRequest {
    pub service_name: Option<String>,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    #[serde(default = "default_bucket_interval")]
    pub bucket_interval: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ToolTimeBucket {
    pub bucket_start: DateTime<Utc>,
    pub tool_name: Option<String>,
    pub tool_type: Option<String>,
    pub call_count: i64,
    pub avg_duration_ms: f64,
    pub error_rate: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ToolDashboardResponse {
    pub aggregates: Vec<GenAiToolActivity>,
    pub time_series: Vec<ToolTimeBucket>,
}

// ── Internal query row (not exposed via API) ──────────────────────────────────

#[derive(Clone, Debug, Default)]
pub struct AgentBucketRow {
    pub bucket_start: DateTime<Utc>,
    pub model: Option<String>,
    pub span_count: i64,
    pub error_count: i64,
    pub error_rate: f64,
    pub avg_duration_ms: f64,
    pub p50_duration_ms: Option<f64>,
    pub p95_duration_ms: Option<f64>,
    pub p99_duration_ms: Option<f64>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_creation_tokens: i64,
    pub cache_read_tokens: i64,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::IntoParams))]
pub struct AgentActivityQuery {
    pub agent_name: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::IntoParams))]
pub struct ConversationQuery {
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::{SpanEvent, SpanId, TraceId};

    fn make_span(attrs: Vec<(&str, serde_json::Value)>) -> TraceSpanRecord {
        make_span_with_events(attrs, vec![])
    }

    fn make_span_with_events(
        attrs: Vec<(&str, serde_json::Value)>,
        events: Vec<SpanEvent>,
    ) -> TraceSpanRecord {
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
            events,
            ..Default::default()
        }
    }

    fn make_event(name: &str, attrs: Vec<(&str, serde_json::Value)>) -> SpanEvent {
        SpanEvent {
            name: name.to_string(),
            timestamp: DateTime::from_timestamp_millis(1_000_500).unwrap(),
            attributes: attrs
                .into_iter()
                .map(|(k, v)| Attribute {
                    key: k.to_string(),
                    value: v,
                })
                .collect(),
            dropped_attributes_count: 0,
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
        assert_eq!(attr_as_i64(&serde_json::Value::Number(42.into())), Some(42));
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
            (
                GEN_AI_AGENT_DESCRIPTION,
                serde_json::Value::String("Helps with math".to_string()),
            ),
            (
                GEN_AI_AGENT_VERSION,
                serde_json::Value::String("1.0.0".to_string()),
            ),
        ]);

        let result = extract_gen_ai_span(&span).expect("should extract");
        assert_eq!(result.agent_name.as_deref(), Some("my-agent"));
        assert_eq!(result.agent_id.as_deref(), Some("agent-123"));
        assert_eq!(result.conversation_id.as_deref(), Some("conv-456"));
        assert_eq!(result.agent_description.as_deref(), Some("Helps with math"));
        assert_eq!(result.agent_version.as_deref(), Some("1.0.0"));
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

    #[test]
    fn test_extract_gen_ai_span_new_request_params() {
        let span = make_span(vec![
            (
                GEN_AI_OPERATION_NAME,
                serde_json::Value::String("chat".to_string()),
            ),
            (GEN_AI_REQUEST_FREQUENCY_PENALTY, serde_json::json!(0.1)),
            (GEN_AI_REQUEST_PRESENCE_PENALTY, serde_json::json!(0.2)),
            (
                GEN_AI_REQUEST_CHOICE_COUNT,
                serde_json::Value::Number(3.into()),
            ),
            (GEN_AI_REQUEST_SEED, serde_json::Value::Number(42.into())),
            (
                GEN_AI_REQUEST_STOP_SEQUENCES,
                serde_json::json!(["<|end|>", "###"]),
            ),
            (
                SERVER_ADDRESS,
                serde_json::Value::String("api.openai.com".to_string()),
            ),
            (SERVER_PORT, serde_json::Value::Number(443.into())),
            (
                GEN_AI_DATA_SOURCE_ID,
                serde_json::Value::String("ds-abc".to_string()),
            ),
        ]);

        let result = extract_gen_ai_span(&span).expect("should extract");
        assert_eq!(result.request_frequency_penalty, Some(0.1));
        assert_eq!(result.request_presence_penalty, Some(0.2));
        assert_eq!(result.request_choice_count, Some(3));
        assert_eq!(result.request_seed, Some(42));
        assert_eq!(result.request_stop_sequences, vec!["<|end|>", "###"]);
        assert_eq!(result.server_address.as_deref(), Some("api.openai.com"));
        assert_eq!(result.server_port, Some(443));
        assert_eq!(result.data_source_id.as_deref(), Some("ds-abc"));
    }

    #[test]
    fn test_extract_opt_in_content_from_span_attrs() {
        let messages_json = r#"[{"role":"user","parts":[{"type":"text","content":"Hello"}]}]"#;
        let span = make_span(vec![
            (
                GEN_AI_OPERATION_NAME,
                serde_json::Value::String("chat".to_string()),
            ),
            (
                GEN_AI_INPUT_MESSAGES,
                serde_json::Value::String(messages_json.to_string()),
            ),
            (
                GEN_AI_OUTPUT_MESSAGES,
                serde_json::Value::String(
                    r#"[{"role":"assistant","parts":[{"type":"text","content":"Hi"}]}]"#
                        .to_string(),
                ),
            ),
        ]);

        let result = extract_gen_ai_span(&span).expect("should extract");
        assert_eq!(result.input_messages.as_deref(), Some(messages_json));
        assert!(result.output_messages.is_some());
    }

    #[test]
    fn test_event_inference_details_fallback() {
        // No opt-in content on span attrs — should fall back to event
        let messages_json = r#"[{"role":"user","parts":[{"type":"text","content":"Hello"}]}]"#;
        let event = make_event(
            GEN_AI_EVENT_INFERENCE_DETAILS,
            vec![
                (
                    GEN_AI_INPUT_MESSAGES,
                    serde_json::Value::String(messages_json.to_string()),
                ),
                (
                    GEN_AI_OUTPUT_MESSAGES,
                    serde_json::Value::String(r#"[{"role":"assistant"}]"#.to_string()),
                ),
            ],
        );
        let span = make_span_with_events(
            vec![(
                GEN_AI_OPERATION_NAME,
                serde_json::Value::String("chat".to_string()),
            )],
            vec![event],
        );

        let result = extract_gen_ai_span(&span).expect("should extract");
        assert_eq!(result.input_messages.as_deref(), Some(messages_json));
        assert!(result.output_messages.is_some());
    }

    #[test]
    fn test_span_attr_wins_over_event() {
        // Span attr present → event value must NOT overwrite it
        let span_messages = r#"[{"role":"user","content":"from-span"}]"#;
        let event_messages = r#"[{"role":"user","content":"from-event"}]"#;
        let event = make_event(
            GEN_AI_EVENT_INFERENCE_DETAILS,
            vec![(
                GEN_AI_INPUT_MESSAGES,
                serde_json::Value::String(event_messages.to_string()),
            )],
        );
        let span = make_span_with_events(
            vec![
                (
                    GEN_AI_OPERATION_NAME,
                    serde_json::Value::String("chat".to_string()),
                ),
                (
                    GEN_AI_INPUT_MESSAGES,
                    serde_json::Value::String(span_messages.to_string()),
                ),
            ],
            vec![event],
        );

        let result = extract_gen_ai_span(&span).expect("should extract");
        assert_eq!(result.input_messages.as_deref(), Some(span_messages));
    }

    #[test]
    fn test_eval_result_extraction() {
        let event = make_event(
            GEN_AI_EVENT_EVALUATION_RESULT,
            vec![
                (
                    GEN_AI_EVALUATION_NAME,
                    serde_json::Value::String("Relevance".to_string()),
                ),
                (GEN_AI_EVALUATION_SCORE_VALUE, serde_json::json!(4.0)),
                (
                    GEN_AI_EVALUATION_SCORE_LABEL,
                    serde_json::Value::String("relevant".to_string()),
                ),
                (
                    GEN_AI_EVALUATION_EXPLANATION,
                    serde_json::Value::String("Good response".to_string()),
                ),
                (
                    GEN_AI_RESPONSE_ID,
                    serde_json::Value::String("chatcmpl-123".to_string()),
                ),
            ],
        );
        let span = make_span_with_events(
            vec![(
                GEN_AI_OPERATION_NAME,
                serde_json::Value::String("chat".to_string()),
            )],
            vec![event],
        );

        let result = extract_gen_ai_span(&span).expect("should extract");
        assert_eq!(result.eval_results.len(), 1);
        let eval = &result.eval_results[0];
        assert_eq!(eval.name, "Relevance");
        assert_eq!(eval.score_value, Some(4.0));
        assert_eq!(eval.score_label.as_deref(), Some("relevant"));
        assert_eq!(eval.explanation.as_deref(), Some("Good response"));
        assert_eq!(eval.response_id.as_deref(), Some("chatcmpl-123"));
    }

    #[test]
    fn test_multiple_eval_events() {
        let event1 = make_event(
            GEN_AI_EVENT_EVALUATION_RESULT,
            vec![(
                GEN_AI_EVALUATION_NAME,
                serde_json::Value::String("Relevance".to_string()),
            )],
        );
        let event2 = make_event(
            GEN_AI_EVENT_EVALUATION_RESULT,
            vec![(
                GEN_AI_EVALUATION_NAME,
                serde_json::Value::String("Correctness".to_string()),
            )],
        );
        let span = make_span_with_events(
            vec![(
                GEN_AI_OPERATION_NAME,
                serde_json::Value::String("chat".to_string()),
            )],
            vec![event1, event2],
        );

        let result = extract_gen_ai_span(&span).expect("should extract");
        assert_eq!(result.eval_results.len(), 2);
        assert_eq!(result.eval_results[0].name, "Relevance");
        assert_eq!(result.eval_results[1].name, "Correctness");
    }

    #[test]
    fn test_eval_event_missing_name_skipped() {
        // gen_ai.evaluation.name is required — event without it must be skipped
        let event = make_event(
            GEN_AI_EVENT_EVALUATION_RESULT,
            vec![(GEN_AI_EVALUATION_SCORE_VALUE, serde_json::json!(1.0))],
        );
        let span = make_span_with_events(
            vec![(
                GEN_AI_OPERATION_NAME,
                serde_json::Value::String("chat".to_string()),
            )],
            vec![event],
        );

        let result = extract_gen_ai_span(&span).expect("should extract");
        assert!(result.eval_results.is_empty());
    }

    #[test]
    fn test_no_events_produces_empty_event_fields() {
        let span = make_span(vec![(
            GEN_AI_OPERATION_NAME,
            serde_json::Value::String("chat".to_string()),
        )]);

        let result = extract_gen_ai_span(&span).expect("should extract");
        assert!(result.input_messages.is_none());
        assert!(result.output_messages.is_none());
        assert!(result.system_instructions.is_none());
        assert!(result.tool_definitions.is_none());
        assert!(result.eval_results.is_empty());
    }

    #[test]
    fn test_parse_stop_sequences_array() {
        let v = serde_json::json!(["<|end|>", "###"]);
        let result = parse_stop_sequences(&v);
        assert_eq!(result, vec!["<|end|>", "###"]);
    }

    #[test]
    fn test_parse_stop_sequences_json_string() {
        let v = serde_json::Value::String(r#"["stop1","stop2"]"#.to_string());
        let result = parse_stop_sequences(&v);
        assert_eq!(result, vec!["stop1", "stop2"]);
    }

    #[test]
    fn test_parse_stop_sequences_invalid_string() {
        let v = serde_json::Value::String("not-an-array".to_string());
        let result = parse_stop_sequences(&v);
        assert!(result.is_empty());
    }
}
