use super::{Attribute, SCOUTER_ENTITY, SpanId, TraceId, TraceSpanRecord};
use scouter_macro::{extract_f64, extract_i64, extract_json_str, extract_str, or_fallback};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Span attribute key constants ─────────────────────────────────────────────
pub const GEN_AI_OPERATION_NAME: &str = "gen_ai.operation.name";
pub const GEN_AI_PROVIDER_NAME: &str = "gen_ai.provider.name";
pub const GEN_AI_SYSTEM: &str = "gen_ai.system";
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
pub const GEN_AI_TOOL_DESCRIPTION: &str = "gen_ai.tool.description";
pub const GEN_AI_TOOL_CALL_ARGUMENTS: &str = "gen_ai.tool.call.arguments";
pub const GEN_AI_TOOL_CALL_RESULT: &str = "gen_ai.tool.call.result";
pub const GEN_AI_REQUEST_STREAM: &str = "gen_ai.request.stream";
pub const GEN_AI_REQUEST_TOP_K: &str = "gen_ai.request.top_k";
pub const GEN_AI_REQUEST_ENCODING_FORMATS: &str = "gen_ai.request.encoding_formats";
pub const GEN_AI_RESPONSE_TIME_TO_FIRST_CHUNK: &str = "gen_ai.response.time_to_first_chunk";
pub const GEN_AI_USAGE_REASONING_OUTPUT_TOKENS: &str = "gen_ai.usage.reasoning.output_tokens";
pub const GEN_AI_PROMPT_NAME: &str = "gen_ai.prompt.name";
pub const GEN_AI_WORKFLOW_NAME: &str = "gen_ai.workflow.name";
pub const GEN_AI_EMBEDDINGS_DIMENSION_COUNT: &str = "gen_ai.embeddings.dimension.count";
pub const GEN_AI_RETRIEVAL_DOCUMENTS: &str = "gen_ai.retrieval.documents";
pub const GEN_AI_RETRIEVAL_QUERY_TEXT: &str = "gen_ai.retrieval.query.text";

pub(crate) const GCP_VERTEX_AGENT_SYSTEM: &str = "gcp.vertex.agent";
pub(crate) const GCP_VERTEX_AGENT_PREFIX: &str = "gcp.vertex.agent.";
pub(crate) const GCP_VERTEX_INVOCATION_ID: &str = "gcp.vertex.agent.invocation_id";
pub(crate) const GCP_VERTEX_SESSION_ID: &str = "gcp.vertex.agent.session_id";
pub(crate) const GCP_VERTEX_EVENT_ID: &str = "gcp.vertex.agent.event_id";
pub(crate) const GCP_VERTEX_LLM_REQUEST: &str = "gcp.vertex.agent.llm_request";
pub(crate) const GCP_VERTEX_LLM_RESPONSE: &str = "gcp.vertex.agent.llm_response";
pub(crate) const GCP_VERTEX_TOOL_CALL_ARGS: &str = "gcp.vertex.agent.tool_call_args";
pub(crate) const GCP_VERTEX_TOOL_RESPONSE: &str = "gcp.vertex.agent.tool_response";
pub(crate) const GCP_VERTEX_DATA: &str = "gcp.vertex.agent.data";
pub(crate) const GCP_MCP_SERVER_DESTINATION_ID: &str = "gcp.mcp.server.destination.id";

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
    /// Name of the evaluation (gen_ai.evaluation.name).
    pub name: String,
    /// Categorical label (e.g. "relevant", "correct"). None if only numeric score present.
    pub score_label: Option<String>,
    /// Numeric evaluation score. None if only a label-based result.
    pub score_value: Option<f64>,
    /// Free-text explanation from the evaluator.
    pub explanation: Option<String>,
    /// Provider response ID this evaluation is linked to (gen_ai.response.id).
    pub response_id: Option<String>,
}

// ── GenAiSpanRecord ───────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiSpanRecord {
    /// OpenTelemetry trace identifier.
    pub trace_id: TraceId,
    /// OpenTelemetry span identifier.
    pub span_id: SpanId,
    /// Service that emitted the span.
    pub service_name: String,
    /// OTel service.namespace resource attribute.
    #[serde(default)]
    pub service_namespace: Option<String>,
    /// OTel service.version resource attribute.
    #[serde(default)]
    pub service_version: Option<String>,
    /// OTel service.instance.id resource attribute.
    #[serde(default)]
    pub service_instance_id: Option<String>,
    /// Span start time (UTC).
    pub start_time: DateTime<Utc>,
    /// Span end time (UTC). None if the span did not record an end.
    pub end_time: Option<DateTime<Utc>>,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: i64,
    /// OTel status code (0 = UNSET, 1 = OK, 2 = ERROR).
    pub status_code: i32,
    /// Operation type (gen_ai.operation.name). e.g. "chat", "invoke_agent", "embedding".
    pub operation_name: Option<String>,
    /// LLM provider (gen_ai.provider.name or gen_ai.system fallback). e.g. "openai", "anthropic".
    pub provider_name: Option<String>,
    /// Model name from the request (gen_ai.request.model).
    pub request_model: Option<String>,
    /// Model name from the response (gen_ai.response.model).
    pub response_model: Option<String>,
    /// Response identifier returned by the provider (gen_ai.response.id).
    pub response_id: Option<String>,
    /// Prompt/input token count (gen_ai.usage.input_tokens).
    pub input_tokens: Option<i64>,
    /// Completion/output token count (gen_ai.usage.output_tokens).
    pub output_tokens: Option<i64>,
    /// Cache creation token count (gen_ai.usage.cache_creation.input_tokens).
    pub cache_creation_input_tokens: Option<i64>,
    /// Cache read token count (gen_ai.usage.cache_read.input_tokens).
    pub cache_read_input_tokens: Option<i64>,
    /// Finish reasons returned by the provider (gen_ai.response.finish_reasons).
    pub finish_reasons: Vec<String>,
    /// Output type (gen_ai.output.type). e.g. "text", "tool_call".
    pub output_type: Option<String>,
    /// Conversation identifier for multi-turn sessions (gen_ai.conversation.id).
    pub conversation_id: Option<String>,
    /// Agent name (gen_ai.agent.name).
    pub agent_name: Option<String>,
    /// Agent identifier (gen_ai.agent.id).
    pub agent_id: Option<String>,
    /// Agent description (gen_ai.agent.description).
    pub agent_description: Option<String>,
    /// Agent version (gen_ai.agent.version).
    pub agent_version: Option<String>,
    /// Data source identifier referenced by the agent (gen_ai.data_source.id).
    pub data_source_id: Option<String>,
    /// Tool name invoked (gen_ai.tool.name).
    pub tool_name: Option<String>,
    /// Tool type (gen_ai.tool.type). e.g. "function", "retrieval".
    pub tool_type: Option<String>,
    /// Tool call identifier (gen_ai.tool.call.id).
    pub tool_call_id: Option<String>,
    /// Request temperature (gen_ai.request.temperature).
    pub request_temperature: Option<f64>,
    /// Maximum tokens for the request (gen_ai.request.max_tokens).
    pub request_max_tokens: Option<i64>,
    /// Nucleus sampling probability (gen_ai.request.top_p).
    pub request_top_p: Option<f64>,
    /// Number of completion choices requested (gen_ai.request.choice.count).
    pub request_choice_count: Option<i64>,
    /// Random seed for deterministic sampling (gen_ai.request.seed).
    pub request_seed: Option<i64>,
    /// Frequency penalty (gen_ai.request.frequency_penalty).
    pub request_frequency_penalty: Option<f64>,
    /// Presence penalty (gen_ai.request.presence_penalty).
    pub request_presence_penalty: Option<f64>,
    /// Stop sequences (gen_ai.request.stop_sequences).
    pub request_stop_sequences: Vec<String>,
    /// Server address (server.address).
    pub server_address: Option<String>,
    /// Server port (server.port).
    pub server_port: Option<i64>,
    /// Error type (error.type attribute on the span).
    pub error_type: Option<String>,
    /// OpenAI API type (openai.api.type).
    pub openai_api_type: Option<String>,
    /// OpenAI service tier (openai.response.service_tier).
    pub openai_service_tier: Option<String>,
    /// Scouter-internal record label.
    pub label: Option<String>,
    /// Scouter entity UID (eval profile or drift profile) associated with this span.
    pub entity_id: Option<String>,
    /// Input messages (gen_ai.input.messages). JSON string. Redacted unless caller has sensitive-content permission.
    pub input_messages: Option<String>,
    /// Output messages (gen_ai.output.messages). JSON string. Redacted unless caller has sensitive-content permission.
    pub output_messages: Option<String>,
    /// System instructions (gen_ai.system_instructions). Redacted unless caller has sensitive-content permission.
    pub system_instructions: Option<String>,
    /// Tool definitions (gen_ai.tool.definitions). JSON string. Redacted unless caller has sensitive-content permission.
    pub tool_definitions: Option<String>,
    /// Tool description (gen_ai.tool.description).
    pub tool_description: Option<String>,
    /// Tool call arguments (gen_ai.tool.call.arguments). JSON string. Redacted unless caller has sensitive-content permission.
    pub tool_call_arguments: Option<String>,
    /// Tool call result (gen_ai.tool.call.result). JSON string. Redacted unless caller has sensitive-content permission.
    pub tool_call_result: Option<String>,
    /// Whether streaming was requested (gen_ai.request.stream).
    pub request_stream: Option<bool>,
    /// Top-k sampling parameter (gen_ai.request.top_k).
    pub request_top_k: Option<f64>,
    /// Requested encoding formats (gen_ai.request.encoding_formats).
    pub request_encoding_formats: Vec<String>,
    /// Time to first streamed chunk in seconds (gen_ai.response.time_to_first_chunk).
    pub response_time_to_first_chunk: Option<f64>,
    /// Reasoning output token count (gen_ai.usage.reasoning.output_tokens).
    pub reasoning_output_tokens: Option<i64>,
    /// Prompt name (gen_ai.prompt.name).
    pub prompt_name: Option<String>,
    /// Workflow name (gen_ai.workflow.name).
    pub workflow_name: Option<String>,
    /// Embedding dimension count (gen_ai.embeddings.dimension.count).
    pub embeddings_dimension_count: Option<i64>,
    /// Retrieval documents (gen_ai.retrieval.documents). JSON string. Redacted unless caller has sensitive-content permission.
    pub retrieval_documents: Option<String>,
    /// Retrieval query text (gen_ai.retrieval.query.text). Redacted unless caller has sensitive-content permission.
    pub retrieval_query_text: Option<String>,
    /// Evaluation results extracted from gen_ai.evaluation.result events.
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

fn attr_as_bool(value: &serde_json::Value) -> Option<bool> {
    match value {
        serde_json::Value::Bool(b) => Some(*b),
        serde_json::Value::String(s) => match s.as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
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

#[derive(Debug, Default)]
pub(crate) struct GenAiFallbackAttributes {
    pub conversation_id: Option<String>,
    pub request_model: Option<String>,
    pub input_messages: Option<String>,
    pub system_instructions: Option<String>,
    pub tool_definitions: Option<String>,
    pub request_top_p: Option<f64>,
    pub request_max_tokens: Option<i64>,
    pub output_messages: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub reasoning_output_tokens: Option<i64>,
    pub finish_reasons: Vec<String>,
    pub tool_call_arguments: Option<String>,
    pub tool_call_result: Option<String>,
}

pub(crate) trait GenAiVendorFallback {
    fn mapped_attributes(&self, record: &TraceSpanRecord) -> GenAiFallbackAttributes;
}

// Selector matches the ADK vendor-instrumentation marker `gcp.vertex.agent`.
// NOTE: this is not the OTel canonical Vertex provider value, which is `gcp.vertex_ai`.
// ADK emits `gcp.vertex.agent` on deprecated `gen_ai.system` plus the
// `gcp.vertex.agent.*` attribute namespace.
pub(crate) fn select_genai_vendor_fallback(
    record: &TraceSpanRecord,
) -> Option<Box<dyn GenAiVendorFallback>> {
    let mut system_match = false;
    let mut prefix_match = false;
    for attr in &record.attributes {
        if attr.key == GEN_AI_SYSTEM
            && let serde_json::Value::String(s) = &attr.value
            && s == GCP_VERTEX_AGENT_SYSTEM
        {
            system_match = true;
        }
        if attr.key.starts_with(GCP_VERTEX_AGENT_PREFIX) {
            prefix_match = true;
        }
        if system_match && prefix_match {
            break;
        }
    }
    if system_match || prefix_match {
        Some(Box::new(GcpVertexFallback))
    } else {
        None
    }
}

struct GcpVertexFallback;

impl GenAiVendorFallback for GcpVertexFallback {
    fn mapped_attributes(&self, record: &TraceSpanRecord) -> GenAiFallbackAttributes {
        let mut out = GenAiFallbackAttributes::default();
        for attr in &record.attributes {
            match attr.key.as_str() {
                GCP_VERTEX_SESSION_ID => {
                    if let serde_json::Value::String(s) = &attr.value {
                        out.conversation_id = Some(s.clone());
                    }
                }
                GCP_VERTEX_TOOL_CALL_ARGS => {
                    out.tool_call_arguments = value_to_json_string(&attr.value);
                }
                GCP_VERTEX_TOOL_RESPONSE => {
                    out.tool_call_result = value_to_json_string(&attr.value);
                }
                GCP_VERTEX_LLM_REQUEST => apply_vertex_llm_request(&attr.value, &mut out),
                GCP_VERTEX_LLM_RESPONSE => apply_vertex_llm_response(&attr.value, &mut out),
                GCP_VERTEX_INVOCATION_ID
                | GCP_VERTEX_EVENT_ID
                | GCP_VERTEX_DATA
                | GCP_MCP_SERVER_DESTINATION_ID => {}
                _ => {}
            }
        }
        out
    }
}

fn vendor_value_to_json(value: &serde_json::Value) -> Option<serde_json::Value> {
    match value {
        serde_json::Value::String(s) => serde_json::from_str(s).ok(),
        serde_json::Value::Object(_) | serde_json::Value::Array(_) => Some(value.clone()),
        _ => None,
    }
}

fn apply_vertex_llm_request(value: &serde_json::Value, out: &mut GenAiFallbackAttributes) {
    let Some(req) = vendor_value_to_json(value) else {
        return;
    };
    if let Some(model) = req.get("model").and_then(|v| v.as_str()) {
        out.request_model = Some(model.to_string());
    }
    if let Some(contents) = req.get("contents") {
        out.input_messages = serde_json::to_string(contents).ok();
    }
    if let Some(config) = req.get("config") {
        if let Some(sys) = config.get("system_instruction") {
            out.system_instructions = serde_json::to_string(sys).ok();
        }
        if let Some(tools) = config.get("tools") {
            out.tool_definitions = serde_json::to_string(tools).ok();
        }
        if let Some(top_p) = config.get("top_p").and_then(|v| v.as_f64()) {
            out.request_top_p = Some(top_p);
        }
        if let Some(max_tokens) = config.get("max_output_tokens").and_then(|v| v.as_i64()) {
            out.request_max_tokens = Some(max_tokens);
        }
    }
}

fn apply_vertex_llm_response(value: &serde_json::Value, out: &mut GenAiFallbackAttributes) {
    let Some(resp) = vendor_value_to_json(value) else {
        return;
    };
    if let Some(content) = resp.get("content") {
        out.output_messages = serde_json::to_string(content).ok();
    }
    if let Some(usage) = resp.get("usage_metadata") {
        if let Some(input) = usage.get("prompt_token_count").and_then(|v| v.as_i64()) {
            out.input_tokens = Some(input);
        }
        if let Some(output) = usage.get("candidates_token_count").and_then(|v| v.as_i64()) {
            out.output_tokens = Some(output);
        }
        if let Some(reasoning) = usage.get("thoughts_token_count").and_then(|v| v.as_i64()) {
            out.reasoning_output_tokens = Some(reasoning);
        }
    }
    if let Some(reason) = resp.get("finish_reason") {
        if let Some(arr) = reason.as_array() {
            out.finish_reasons = arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
        } else if let Some(s) = reason.as_str() {
            out.finish_reasons = vec![s.to_string()];
        }
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
        service_namespace: record.service_namespace.clone(),
        service_version: record.service_version.clone(),
        service_instance_id: record.service_instance_id.clone(),
        start_time: record.start_time,
        end_time: Some(record.end_time),
        duration_ms: record.duration_ms,
        status_code: record.status_code,
        label: record.label.clone(),
        ..Default::default()
    };

    for Attribute { key, value } in &record.attributes {
        match key.as_str() {
            GEN_AI_OPERATION_NAME => extract_str!(out, operation_name, value),
            GEN_AI_PROVIDER_NAME => extract_str!(out, provider_name, value),
            GEN_AI_SYSTEM if out.provider_name.is_none() => {
                if let serde_json::Value::String(s) = value {
                    out.provider_name = Some(s.clone());
                }
            }
            GEN_AI_REQUEST_MODEL => extract_str!(out, request_model, value),
            GEN_AI_RESPONSE_MODEL => extract_str!(out, response_model, value),
            GEN_AI_RESPONSE_ID => extract_str!(out, response_id, value),
            GEN_AI_USAGE_INPUT_TOKENS => extract_i64!(out, input_tokens, value),
            GEN_AI_USAGE_OUTPUT_TOKENS => extract_i64!(out, output_tokens, value),
            GEN_AI_USAGE_CACHE_CREATION_INPUT_TOKENS => extract_i64!(out, cache_creation_input_tokens, value),
            GEN_AI_USAGE_CACHE_READ_INPUT_TOKENS => extract_i64!(out, cache_read_input_tokens, value),
            GEN_AI_RESPONSE_FINISH_REASONS => {
                out.finish_reasons = parse_finish_reasons(value);
            }
            GEN_AI_OUTPUT_TYPE => extract_str!(out, output_type, value),
            GEN_AI_CONVERSATION_ID => extract_str!(out, conversation_id, value),
            GEN_AI_AGENT_NAME => extract_str!(out, agent_name, value),
            GEN_AI_AGENT_ID => extract_str!(out, agent_id, value),
            GEN_AI_AGENT_DESCRIPTION => extract_str!(out, agent_description, value),
            GEN_AI_AGENT_VERSION => extract_str!(out, agent_version, value),
            GEN_AI_DATA_SOURCE_ID => extract_str!(out, data_source_id, value),
            GEN_AI_TOOL_NAME => extract_str!(out, tool_name, value),
            GEN_AI_TOOL_TYPE => extract_str!(out, tool_type, value),
            GEN_AI_TOOL_CALL_ID => extract_str!(out, tool_call_id, value),
            GEN_AI_REQUEST_TEMPERATURE => extract_f64!(out, request_temperature, value),
            GEN_AI_REQUEST_MAX_TOKENS => extract_i64!(out, request_max_tokens, value),
            GEN_AI_REQUEST_TOP_P => extract_f64!(out, request_top_p, value),
            GEN_AI_REQUEST_CHOICE_COUNT => extract_i64!(out, request_choice_count, value),
            GEN_AI_REQUEST_SEED => extract_i64!(out, request_seed, value),
            GEN_AI_REQUEST_FREQUENCY_PENALTY => extract_f64!(out, request_frequency_penalty, value),
            GEN_AI_REQUEST_PRESENCE_PENALTY => extract_f64!(out, request_presence_penalty, value),
            GEN_AI_REQUEST_STOP_SEQUENCES => {
                out.request_stop_sequences = parse_stop_sequences(value);
            }
            SERVER_ADDRESS => extract_str!(out, server_address, value),
            SERVER_PORT => extract_i64!(out, server_port, value),
            GEN_AI_ERROR_TYPE => extract_str!(out, error_type, value),
            OPENAI_API_TYPE => extract_str!(out, openai_api_type, value),
            OPENAI_SERVICE_TIER => extract_str!(out, openai_service_tier, value),
            GEN_AI_INPUT_MESSAGES => extract_json_str!(out, input_messages, value),
            GEN_AI_OUTPUT_MESSAGES => extract_json_str!(out, output_messages, value),
            GEN_AI_SYSTEM_INSTRUCTIONS => extract_json_str!(out, system_instructions, value),
            GEN_AI_TOOL_DEFINITIONS => extract_json_str!(out, tool_definitions, value),
            GEN_AI_TOOL_DESCRIPTION => extract_str!(out, tool_description, value),
            GEN_AI_TOOL_CALL_ARGUMENTS => extract_json_str!(out, tool_call_arguments, value),
            GEN_AI_TOOL_CALL_RESULT => extract_json_str!(out, tool_call_result, value),
            GEN_AI_REQUEST_STREAM => {
                out.request_stream = attr_as_bool(value);
            }
            GEN_AI_REQUEST_TOP_K => extract_f64!(out, request_top_k, value),
            GEN_AI_REQUEST_ENCODING_FORMATS => {
                out.request_encoding_formats = parse_stop_sequences(value);
            }
            GEN_AI_RESPONSE_TIME_TO_FIRST_CHUNK => extract_f64!(out, response_time_to_first_chunk, value),
            GEN_AI_USAGE_REASONING_OUTPUT_TOKENS => extract_i64!(out, reasoning_output_tokens, value),
            GEN_AI_PROMPT_NAME => extract_str!(out, prompt_name, value),
            GEN_AI_WORKFLOW_NAME => extract_str!(out, workflow_name, value),
            GEN_AI_EMBEDDINGS_DIMENSION_COUNT => extract_i64!(out, embeddings_dimension_count, value),
            GEN_AI_RETRIEVAL_DOCUMENTS => extract_json_str!(out, retrieval_documents, value),
            GEN_AI_RETRIEVAL_QUERY_TEXT => extract_json_str!(out, retrieval_query_text, value),
            _ => {}
        }
    }

    out.operation_name.as_ref()?;

    // Extract entity_id: span attributes first (stamped at creation via default_entity_uid or
    // active_profile baggage), then event attributes as fallback (queue-item path).
    out.entity_id = record
        .attributes
        .iter()
        .find(|a| a.key.starts_with(SCOUTER_ENTITY))
        .and_then(|a| {
            if let serde_json::Value::String(s) = &a.value {
                Some(s.clone())
            } else {
                None
            }
        });
    if out.entity_id.is_none() {
        out.entity_id = record
            .events
            .iter()
            .flat_map(|e| e.attributes.iter())
            .find(|a| a.key.starts_with(SCOUTER_ENTITY))
            .and_then(|a| {
                if let serde_json::Value::String(s) = &a.value {
                    Some(s.clone())
                } else {
                    None
                }
            });
    }

    // Scalars: span attrs authoritative. Opt-in content: span wins, event fallback.
    // Eval results always come from events.
    let mut event_data = extract_gen_ai_events(record);
    or_fallback!(out, event_data, input_messages, output_messages, system_instructions, tool_definitions);
    out.eval_results = event_data.eval_results;

    if let Some(fallback) = select_genai_vendor_fallback(record) {
        let mut vendor = fallback.mapped_attributes(record);
        or_fallback!(
            out, vendor,
            conversation_id, request_model, input_messages, system_instructions,
            tool_definitions, request_top_p, request_max_tokens, output_messages,
            input_tokens, output_tokens, reasoning_output_tokens,
            tool_call_arguments, tool_call_result,
        );
        if out.finish_reasons.is_empty() {
            out.finish_reasons = vendor.finish_reasons;
        }
    }

    Some(out)
}

// ── Response / aggregation types ──────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiTokenBucket {
    /// Bucket start time (UTC, truncated to the requested granularity).
    pub bucket_start: DateTime<Utc>,
    /// Sum of prompt/input tokens in this bucket.
    pub total_input_tokens: i64,
    /// Sum of completion/output tokens in this bucket.
    pub total_output_tokens: i64,
    /// Sum of cache creation tokens in this bucket.
    pub total_cache_creation_tokens: i64,
    /// Sum of cache read tokens in this bucket.
    pub total_cache_read_tokens: i64,
    /// Number of GenAI spans in this bucket.
    pub span_count: i64,
    /// Fraction of spans in this bucket that have a non-zero error status.
    pub error_rate: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiOperationBreakdown {
    /// Operation type (gen_ai.operation.name). e.g. "chat", "invoke_agent", "embedding".
    pub operation_name: String,
    /// Provider associated with this operation. None = mixed or unknown.
    pub provider_name: Option<String>,
    /// Total spans for this operation in the query window.
    pub span_count: i64,
    /// Mean response duration in milliseconds for this operation.
    pub avg_duration_ms: f64,
    /// Total input tokens consumed by this operation.
    pub total_input_tokens: i64,
    /// Total output tokens produced by this operation.
    pub total_output_tokens: i64,
    /// Fraction of spans with a non-zero error status.
    pub error_rate: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiModelUsage {
    /// Response model name (gen_ai.response.model).
    pub model: String,
    /// Provider that served this model. None if not recorded.
    pub provider_name: Option<String>,
    /// Total spans that used this model in the query window.
    pub span_count: i64,
    /// Total input tokens consumed by this model.
    pub total_input_tokens: i64,
    /// Total output tokens produced by this model.
    pub total_output_tokens: i64,
    /// Median (p50) response duration in milliseconds.
    pub p50_duration_ms: Option<f64>,
    /// 95th-percentile response duration in milliseconds.
    pub p95_duration_ms: Option<f64>,
    /// Fraction of spans with a non-zero error status.
    pub error_rate: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiAgentActivity {
    /// Agent name (gen_ai.agent.name). None if not set on the span.
    pub agent_name: Option<String>,
    /// Agent identifier (gen_ai.agent.id). None if not set on the span.
    pub agent_id: Option<String>,
    /// Conversation identifier (gen_ai.conversation.id). None if not set on the span.
    pub conversation_id: Option<String>,
    /// Total spans attributed to this agent in the query window.
    pub span_count: i64,
    /// Total input tokens consumed by this agent.
    pub total_input_tokens: i64,
    /// Total output tokens produced by this agent.
    pub total_output_tokens: i64,
    /// Timestamp of the most recent span for this agent.
    pub last_seen: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiToolActivity {
    /// Tool name (gen_ai.tool.name). None if not set on the span.
    pub tool_name: Option<String>,
    /// Tool type (gen_ai.tool.type). e.g. "function", "retrieval". None if not set.
    pub tool_type: Option<String>,
    /// Total number of tool call spans in the query window.
    pub call_count: i64,
    /// Mean duration of tool calls in milliseconds.
    pub avg_duration_ms: f64,
    /// Fraction of tool calls with a non-zero error status.
    pub error_rate: f64,
}

fn default_bucket_interval() -> String {
    "hour".to_string()
}

fn default_trace_span_limit() -> usize {
    500
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiMetricsRequest {
    /// Service the request is scoped to. None = caller's default service.
    pub service_name: Option<String>,
    pub service_namespace: Option<String>,
    pub service_version: Option<String>,
    pub service_instance_id: Option<String>,
    /// Window start (UTC, inclusive).
    pub start_time: DateTime<Utc>,
    /// Window end (UTC, exclusive).
    pub end_time: DateTime<Utc>,
    /// Time-series granularity. One of: second, minute, hour, day, week, month, year.
    #[serde(default = "default_bucket_interval")]
    pub bucket_interval: String,
    /// Filter to one operation type (e.g. "invoke_agent", "chat", "embedding").
    pub operation_name: Option<String>,
    /// Filter to one provider (e.g. "openai", "anthropic", "google").
    pub provider_name: Option<String>,
    /// Filter to one response model (e.g. "gpt-4o", "claude-opus-4-7").
    pub model: Option<String>,
    /// Filter to one agent (matches agent_name attribute on the GenAI span).
    pub agent_name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiSpanFilters {
    /// Filter to spans from this service. None = all services.
    pub service_name: Option<String>,
    pub service_namespace: Option<String>,
    pub service_version: Option<String>,
    pub service_instance_id: Option<String>,
    /// Window start (UTC, inclusive). None = no lower bound.
    pub start_time: Option<DateTime<Utc>>,
    /// Window end (UTC, exclusive). None = no upper bound.
    pub end_time: Option<DateTime<Utc>>,
    /// Filter to one operation type (e.g. "chat", "invoke_agent").
    pub operation_name: Option<String>,
    /// Filter to one provider (e.g. "openai", "anthropic").
    pub provider_name: Option<String>,
    /// Filter to one response model.
    pub model: Option<String>,
    /// Filter to spans belonging to one conversation.
    pub conversation_id: Option<String>,
    /// Filter to spans from one agent.
    pub agent_name: Option<String>,
    /// Filter to spans that invoked this tool.
    pub tool_name: Option<String>,
    /// Filter to spans with this error type.
    pub error_type: Option<String>,
    /// Maximum number of spans to return. None = server default.
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiErrorCount {
    /// Error type string (error.type attribute on the span).
    pub error_type: String,
    /// Number of spans with this error type in the query window.
    pub count: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiTokenMetricsResponse {
    /// Time-series token buckets, one per time interval.
    pub buckets: Vec<GenAiTokenBucket>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiOperationBreakdownResponse {
    /// Per-operation aggregates for the query window.
    pub operations: Vec<GenAiOperationBreakdown>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiModelUsageResponse {
    /// Per-model aggregates for the query window.
    pub models: Vec<GenAiModelUsage>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiAgentActivityResponse {
    /// Per-agent activity rows for the query window.
    pub agents: Vec<GenAiAgentActivity>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiToolActivityResponse {
    /// Per-tool activity rows for the query window.
    pub tools: Vec<GenAiToolActivity>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiErrorBreakdownResponse {
    /// Error counts grouped by error type.
    pub errors: Vec<GenAiErrorCount>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiSpansResponse {
    /// Individual GenAI spans matching the query filters.
    pub spans: Vec<GenAiSpanRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiTraceMetricsRequest {
    /// Window start (UTC, inclusive). None = 30 minutes before end_time.
    pub start_time: Option<DateTime<Utc>>,
    /// Window end (UTC, exclusive). None = now.
    pub end_time: Option<DateTime<Utc>>,
    /// Time-series granularity. One of: second, minute, hour, day, week, month, year.
    #[serde(default = "default_bucket_interval")]
    pub bucket_interval: String,
    /// Optional pricing table for cost computation. Keyed by response_model. Empty = no cost computation.
    #[serde(default)]
    pub model_pricing: HashMap<String, ModelPricing>,
    /// Maximum number of spans to include in the `spans` list. Clamped to 5000. Default: 500.
    #[serde(default = "default_trace_span_limit")]
    pub span_limit: usize,
    /// When true, include sensitive content fields (input/output messages, system instructions, tool definitions). Requires admin permission.
    #[serde(default)]
    pub include_sensitive_content: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiTraceMetricsResponse {
    /// Trace ID this response covers.
    pub trace_id: String,
    /// True if the trace contains at least one GenAI span.
    pub has_genai_spans: bool,
    /// GenAI spans in this trace, truncated to `span_limit`.
    pub spans: Vec<GenAiSpanRecord>,
    /// The span limit that was applied (clamped to 5000).
    pub span_limit: usize,
    /// True if the actual span count exceeded `span_limit`.
    pub spans_truncated: bool,
    /// True if sensitive content fields were redacted (caller lacked permission or did not request them).
    pub sensitive_content_redacted: bool,
    /// Token usage time-series for this trace.
    pub token_metrics: GenAiTokenMetricsResponse,
    /// Per-operation aggregates for this trace.
    pub operation_breakdown: GenAiOperationBreakdownResponse,
    /// Per-model aggregates for this trace.
    pub model_usage: GenAiModelUsageResponse,
    /// Per-agent activity rows for this trace.
    pub agent_activity: GenAiAgentActivityResponse,
    /// Agent time-series dashboard for this trace.
    pub agent_dashboard: AgentDashboardResponse,
    /// Tool call aggregates and time-series for this trace.
    pub tool_dashboard: ToolDashboardResponse,
    /// Error breakdown by type for this trace.
    pub error_breakdown: GenAiErrorBreakdownResponse,
}

// ── Agent dashboard ───────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ModelPricing {
    /// Cost per million input tokens (USD).
    pub input_per_million: f64,
    /// Cost per million output tokens (USD).
    pub output_per_million: f64,
    /// Cost per million cache creation tokens (USD).
    pub cache_creation_per_million: f64,
    /// Cost per million cache read tokens (USD).
    pub cache_read_per_million: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AgentDashboardRequest {
    /// Service to scope the dashboard to. None = all services.
    pub service_name: Option<String>,
    #[serde(default)]
    pub service_namespace: Option<String>,
    #[serde(default)]
    pub service_version: Option<String>,
    #[serde(default)]
    pub service_instance_id: Option<String>,
    /// Scouter entity UID (AgentEvalProfile or DriftProfile) to scope the dashboard to.
    /// None = all entities within the service. Set this when navigating from an eval profile page.
    pub entity_id: Option<String>,
    /// Window start (UTC, inclusive).
    pub start_time: DateTime<Utc>,
    /// Window end (UTC, exclusive). Must be > start_time.
    pub end_time: DateTime<Utc>,
    /// Time-series granularity. One of: second, minute, hour, day, week, month, year.
    #[serde(default = "default_bucket_interval")]
    pub bucket_interval: String,
    /// Filter to one agent (matches gen_ai.agent.name). None = all agents.
    pub agent_name: Option<String>,
    /// Filter to one provider (e.g. "openai", "anthropic", "google"). None = all providers.
    pub provider_name: Option<String>,
    /// Optional pricing table for cost computation. Keyed by response_model. Empty = no cost computation.
    /// The key `"unknown"` is reserved for spans with no model attribution — do not include it as a pricing key.
    #[serde(default)]
    pub model_pricing: std::collections::HashMap<String, ModelPricing>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AgentMetricBucket {
    /// Bucket start time (UTC, truncated to the requested granularity).
    pub bucket_start: DateTime<Utc>,
    /// Number of agent spans in this bucket.
    pub span_count: i64,
    /// Number of spans with a non-zero error status in this bucket.
    pub error_count: i64,
    /// Fraction of spans with a non-zero error status.
    pub error_rate: f64,
    /// Mean response duration in milliseconds for spans in this bucket.
    pub avg_duration_ms: f64,
    /// Median (p50) response duration in milliseconds.
    pub p50_duration_ms: Option<f64>,
    /// 95th-percentile response duration in milliseconds.
    pub p95_duration_ms: Option<f64>,
    /// 99th-percentile response duration in milliseconds.
    pub p99_duration_ms: Option<f64>,
    /// Total input tokens across all spans in this bucket.
    pub total_input_tokens: i64,
    /// Total output tokens across all spans in this bucket.
    pub total_output_tokens: i64,
    /// Total cache creation tokens across all spans in this bucket.
    pub total_cache_creation_tokens: i64,
    /// Total cache read tokens across all spans in this bucket.
    pub total_cache_read_tokens: i64,
    /// Per-bucket cost. Always None — cost is only available in `summary.cost_by_model`.
    pub total_cost: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ModelCostBreakdown {
    /// Response model name.
    pub model: String,
    /// Total input tokens for this model in the query window.
    pub total_input_tokens: i64,
    /// Total output tokens for this model in the query window.
    pub total_output_tokens: i64,
    /// Total cache creation tokens for this model in the query window.
    pub total_cache_creation_tokens: i64,
    /// Total cache read tokens for this model in the query window.
    pub total_cache_read_tokens: i64,
    /// Total cost in USD. None if no pricing was supplied for this model.
    pub total_cost: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AgentDashboardSummary {
    /// Total agent span requests in the query window.
    pub total_requests: i64,
    /// Mean response duration across all buckets in milliseconds.
    pub avg_duration_ms: f64,
    /// Median (p50) response duration over the full query window in milliseconds.
    pub p50_duration_ms: Option<f64>,
    /// 95th-percentile response duration over the full query window in milliseconds.
    pub p95_duration_ms: Option<f64>,
    /// 99th-percentile response duration over the full query window in milliseconds.
    pub p99_duration_ms: Option<f64>,
    /// Fraction of all spans with a non-zero error status.
    pub overall_error_rate: f64,
    /// Sum of input tokens across all spans in the query window.
    pub total_input_tokens: i64,
    /// Sum of output tokens across all spans in the query window.
    pub total_output_tokens: i64,
    /// Sum of cache creation tokens across all spans in the query window.
    pub total_cache_creation_tokens: i64,
    /// Sum of cache read tokens across all spans in the query window.
    pub total_cache_read_tokens: i64,
    /// Number of distinct agent_name values in the query window.
    pub unique_agent_count: i64,
    /// Number of distinct conversation_id values in the query window.
    pub unique_conversation_count: i64,
    /// Per-model token totals and computed cost (if pricing was supplied).
    pub cost_by_model: Vec<ModelCostBreakdown>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AgentDashboardResponse {
    /// Summary aggregates across the full query window.
    pub summary: AgentDashboardSummary,
    /// Time-series metric buckets, one per time interval.
    pub buckets: Vec<AgentMetricBucket>,
}

// ── Tool dashboard ────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ToolDashboardRequest {
    /// Service the request is scoped to. None = all services.
    pub service_name: Option<String>,
    #[serde(default)]
    pub service_namespace: Option<String>,
    #[serde(default)]
    pub service_version: Option<String>,
    #[serde(default)]
    pub service_instance_id: Option<String>,
    /// Window start (UTC, inclusive).
    pub start_time: DateTime<Utc>,
    /// Window end (UTC, exclusive).
    pub end_time: DateTime<Utc>,
    /// Time-series granularity. One of: second, minute, hour, day, week, month, year.
    #[serde(default = "default_bucket_interval")]
    pub bucket_interval: String,
    /// Filter to one agent (matches agent_name attribute on the GenAI span).
    pub agent_name: Option<String>,
    /// Filter to one provider (e.g. "openai", "anthropic", "google").
    pub provider_name: Option<String>,
    /// Filter to one model (matches response_model on the GenAI span).
    pub model: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ToolTimeBucket {
    /// Bucket start time (UTC, truncated to the requested granularity).
    pub bucket_start: DateTime<Utc>,
    /// Tool name (gen_ai.tool.name). None if not set on the span.
    pub tool_name: Option<String>,
    /// Tool type (gen_ai.tool.type). None if not set on the span.
    pub tool_type: Option<String>,
    /// Number of tool call spans in this bucket.
    pub call_count: i64,
    /// Mean duration of tool calls in this bucket, in milliseconds.
    pub avg_duration_ms: f64,
    /// Fraction of tool calls in this bucket with a non-zero error status.
    pub error_rate: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ToolDashboardResponse {
    /// Per-tool aggregates across the full query window.
    pub aggregates: Vec<GenAiToolActivity>,
    /// Time-series tool call data, one bucket per time interval per tool.
    pub time_series: Vec<ToolTimeBucket>,
}

// ── Internal query row (not exposed via API) ──────────────────────────────────

#[derive(Clone, Debug, Default)]
pub struct AgentBucketRow {
    pub bucket_start: DateTime<Utc>,
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

#[derive(Clone, Debug, Default)]
pub struct AgentModelCostRow {
    pub model: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_creation_tokens: i64,
    pub cache_read_tokens: i64,
}

// ── Composite dashboard types ─────────────────────────────────────────────────

pub const GENAI_DASHBOARD_SCHEMA_VERSION: u32 = 1;

/// Echoed back in every dashboard response. Mirrors the request body so
/// callers (UIs, agents) can confirm scope without tracking request state
/// across parallel calls.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AppliedFilters {
    /// Service the dashboard is scoped to.
    pub service_name: Option<String>,
    pub service_namespace: Option<String>,
    pub service_version: Option<String>,
    pub service_instance_id: Option<String>,
    /// Entity UID filter applied. None = all entities.
    pub entity_id: Option<String>,
    /// Agent drilldown applied. None = service-wide view.
    pub agent_name: Option<String>,
    /// Provider filter applied.
    pub provider_name: Option<String>,
    /// Operation filter applied.
    pub operation_name: Option<String>,
    /// Response model filter applied.
    pub model: Option<String>,
    /// Window start (UTC, inclusive).
    pub start_time: DateTime<Utc>,
    /// Window end (UTC, exclusive).
    pub end_time: DateTime<Utc>,
    /// Time-series granularity used.
    pub bucket_interval: String,
}

/// Distinct filter values present in the query window. Always computed
/// service-scoped (NOT narrowed by `agent_name`) so UI dropdowns stay
/// populated when a user drills into a single agent.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct AvailableFilters {
    /// Per-agent activity rows. Source for the "Select agent" dropdown.
    /// Always service-scoped — unchanged by agent drilldown.
    pub agents: Vec<GenAiAgentActivity>,
    /// Distinct provider_name values present in the window.
    pub providers: Vec<String>,
    /// Distinct response_model values present in the window.
    pub models: Vec<String>,
    /// Distinct operation_name values present in the window.
    pub operations: Vec<String>,
}

/// Response-level metadata. Use for cache invalidation (`generated_at`),
/// breaking-change negotiation (`schema_version`), and empty-vs-no-traffic
/// disambiguation (`total_spans`).
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct DashboardMetadata {
    /// Server timestamp when response was generated (UTC).
    pub generated_at: DateTime<Utc>,
    /// Response schema version. Bump on breaking changes. Currently 1.
    pub schema_version: u32,
    /// Total GenAI spans matched by the applied filters in the window.
    /// Use to distinguish "no data" from "no matching filter".
    pub total_spans: i64,
}

/// Internal row returned by the distinct-filter-values query.
/// Not part of the public API — assembled into `AvailableFilters` by the handler.
#[derive(Clone, Debug, Default)]
pub struct DistinctFilterValues {
    pub providers: Vec<String>,
    pub models: Vec<String>,
    pub operations: Vec<String>,
}

/// Request body for the composite GenAI dashboard endpoint.
///
/// Use `agent_name = None` for a service-wide view.
/// Set `agent_name` to drill into a single agent's metrics.
/// The `available_filters` in the response always reflects the full service window
/// so UI dropdowns stay populated during drilldown.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiDashboardRequest {
    /// Service to scope the dashboard to. None = all services.
    pub service_name: Option<String>,
    pub service_namespace: Option<String>,
    pub service_version: Option<String>,
    pub service_instance_id: Option<String>,
    /// Scouter entity UID (AgentEvalProfile or DriftProfile) to scope the dashboard to.
    /// None = all entities within the service. Set this when navigating from an eval profile page.
    pub entity_id: Option<String>,
    /// Window start (UTC, inclusive).
    pub start_time: DateTime<Utc>,
    /// Window end (UTC, exclusive). Must be > start_time. Window cannot exceed 30 days.
    pub end_time: DateTime<Utc>,
    /// Time-series granularity. One of: second, minute, hour, day, week, month, year.
    #[serde(default = "default_bucket_interval")]
    pub bucket_interval: String,
    /// Drill into a single agent. None = service-wide view.
    pub agent_name: Option<String>,
    /// Filter to one provider (e.g. "openai", "anthropic", "google").
    pub provider_name: Option<String>,
    /// Filter to one operation type (e.g. "invoke_agent", "chat", "embedding").
    pub operation_name: Option<String>,
    /// Filter to one response model (e.g. "gpt-4o", "claude-opus-4-7").
    pub model: Option<String>,
    /// Optional pricing table for cost computation. Keyed by response_model.
    /// Empty = no cost computation. Cost appears in `agent_dashboard.summary.cost_by_model`.
    /// The key `"unknown"` is reserved for spans with no model attribution — do not include it as a pricing key.
    #[serde(default)]
    pub model_pricing: HashMap<String, ModelPricing>,
}

/// Composite GenAI dashboard for a service or agent slice.
///
/// Returned by `POST /scouter/genai/dashboard`. Single round-trip — populates
/// every panel a UI dashboard needs. Use `applied_filters` to confirm scope,
/// `available_filters` to populate dropdowns, `metadata` for staleness.
///
/// **Empty-state invariant**: When no spans match the applied filters, all
/// `Vec` fields are empty (`[]`), all numeric aggregates are `0`, all
/// `Option<f64>` percentiles are `None`. `metadata.total_spans` is `0`.
/// `available_filters.agents` MAY still be populated if other agents exist
/// in the service+window.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct GenAiDashboardResponse {
    /// Filters the server applied. Mirrors the request body.
    pub applied_filters: AppliedFilters,
    /// All distinct filter values present in the query window. Use to populate dropdowns.
    /// Always service-scoped — not narrowed by `agent_name`.
    pub available_filters: AvailableFilters,
    /// Query metadata: generated_at, schema_version, total_spans.
    pub metadata: DashboardMetadata,
    /// Token usage time-series. Filtered by applied_filters.
    pub token_metrics: GenAiTokenMetricsResponse,
    /// Per-operation×provider aggregates. Filtered by applied_filters.
    pub operation_breakdown: GenAiOperationBreakdownResponse,
    /// Per-model token and cost aggregates. Filtered by applied_filters.
    pub model_usage: GenAiModelUsageResponse,
    /// Agent time-series and summary. Filtered by applied_filters.
    pub agent_dashboard: AgentDashboardResponse,
    /// Tool call aggregates and time-series. Filtered by applied_filters.
    pub tool_dashboard: ToolDashboardResponse,
    /// Error breakdown by type. Filtered by applied_filters.
    pub error_breakdown: GenAiErrorBreakdownResponse,
    /// True if any time-series panel was capped at MAX_DASHBOARD_BUCKETS.
    /// Caller should widen `bucket_interval` or shorten the window.
    /// Note: `agent_dashboard.summary` token totals always reflect the full window, even when
    /// agent buckets were truncated, because cost aggregates are computed independently.
    pub buckets_truncated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::IntoParams))]
pub struct AgentActivityQuery {
    /// Filter to spans from one agent (matches gen_ai.agent.name). None = all agents.
    pub agent_name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::IntoParams))]
pub struct ConversationQuery {
    /// Window start (UTC, RFC 3339). None = no lower bound.
    pub start_time: Option<String>,
    /// Window end (UTC, RFC 3339). None = no upper bound.
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
    fn test_extract_gen_ai_span_uses_system_as_provider_fallback() {
        let span = make_span(vec![
            (
                GEN_AI_OPERATION_NAME,
                serde_json::Value::String("generate_content".to_string()),
            ),
            (
                GEN_AI_SYSTEM,
                serde_json::Value::String("gemini".to_string()),
            ),
        ]);

        let result = extract_gen_ai_span(&span).expect("should extract");
        assert_eq!(result.provider_name.as_deref(), Some("gemini"));
    }

    #[test]
    fn test_extract_gen_ai_span_keeps_provider_name_over_system() {
        let span = make_span(vec![
            (
                GEN_AI_OPERATION_NAME,
                serde_json::Value::String("generate_content".to_string()),
            ),
            (
                GEN_AI_PROVIDER_NAME,
                serde_json::Value::String("google".to_string()),
            ),
            (
                GEN_AI_SYSTEM,
                serde_json::Value::String("gemini".to_string()),
            ),
        ]);

        let result = extract_gen_ai_span(&span).expect("should extract");
        assert_eq!(result.provider_name.as_deref(), Some("google"));
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
    fn test_extract_new_standard_fields_all_present() {
        let span = make_span(vec![
            (GEN_AI_OPERATION_NAME, serde_json::json!("embedding")),
            (GEN_AI_TOOL_DESCRIPTION, serde_json::json!("Searches docs")),
            (
                GEN_AI_TOOL_CALL_ARGUMENTS,
                serde_json::json!({"query": "otel"}),
            ),
            (GEN_AI_TOOL_CALL_RESULT, serde_json::json!({"count": 2})),
            (GEN_AI_REQUEST_STREAM, serde_json::json!(true)),
            (GEN_AI_REQUEST_TOP_K, serde_json::json!(40.0)),
            (
                GEN_AI_REQUEST_ENCODING_FORMATS,
                serde_json::json!(["float", "base64"]),
            ),
            (GEN_AI_RESPONSE_TIME_TO_FIRST_CHUNK, serde_json::json!(0.25)),
            (GEN_AI_USAGE_REASONING_OUTPUT_TOKENS, serde_json::json!(11)),
            (GEN_AI_PROMPT_NAME, serde_json::json!("planner")),
            (GEN_AI_WORKFLOW_NAME, serde_json::json!("agent-loop")),
            (GEN_AI_EMBEDDINGS_DIMENSION_COUNT, serde_json::json!(1536)),
            (
                GEN_AI_RETRIEVAL_DOCUMENTS,
                serde_json::json!([{"id": "doc-1"}]),
            ),
            (
                GEN_AI_RETRIEVAL_QUERY_TEXT,
                serde_json::json!("what is otel"),
            ),
        ]);

        let result = extract_gen_ai_span(&span).expect("should extract");
        assert_eq!(result.tool_description.as_deref(), Some("Searches docs"));
        assert_eq!(
            result.tool_call_arguments.as_deref(),
            Some(r#"{"query":"otel"}"#)
        );
        assert_eq!(result.tool_call_result.as_deref(), Some(r#"{"count":2}"#));
        assert_eq!(result.request_stream, Some(true));
        assert_eq!(result.request_top_k, Some(40.0));
        assert_eq!(result.request_encoding_formats, vec!["float", "base64"]);
        assert_eq!(result.response_time_to_first_chunk, Some(0.25));
        assert_eq!(result.reasoning_output_tokens, Some(11));
        assert_eq!(result.prompt_name.as_deref(), Some("planner"));
        assert_eq!(result.workflow_name.as_deref(), Some("agent-loop"));
        assert_eq!(result.embeddings_dimension_count, Some(1536));
        assert_eq!(
            result.retrieval_documents.as_deref(),
            Some(r#"[{"id":"doc-1"}]"#)
        );
        assert_eq!(result.retrieval_query_text.as_deref(), Some("what is otel"));
    }

    #[test]
    fn test_request_stream_bool_parsing() {
        assert_eq!(attr_as_bool(&serde_json::json!(true)), Some(true));
        assert_eq!(attr_as_bool(&serde_json::json!(false)), Some(false));
        assert_eq!(attr_as_bool(&serde_json::json!("true")), Some(true));
        assert_eq!(attr_as_bool(&serde_json::json!("false")), Some(false));
        assert_eq!(attr_as_bool(&serde_json::json!("TRUE")), None);
    }

    #[test]
    fn test_request_encoding_formats_array_and_json_string() {
        let array_span = make_span(vec![
            (GEN_AI_OPERATION_NAME, serde_json::json!("embedding")),
            (
                GEN_AI_REQUEST_ENCODING_FORMATS,
                serde_json::json!(["float", "base64"]),
            ),
        ]);
        let array_result = extract_gen_ai_span(&array_span).expect("should extract");
        assert_eq!(
            array_result.request_encoding_formats,
            vec!["float", "base64"]
        );

        let string_span = make_span(vec![
            (GEN_AI_OPERATION_NAME, serde_json::json!("embedding")),
            (
                GEN_AI_REQUEST_ENCODING_FORMATS,
                serde_json::json!(r#"["float","base64"]"#),
            ),
        ]);
        let string_result = extract_gen_ai_span(&string_span).expect("should extract");
        assert_eq!(
            string_result.request_encoding_formats,
            vec!["float", "base64"]
        );
    }

    #[test]
    fn test_select_vendor_fallback_via_system_attr() {
        let span = make_span(vec![(
            GEN_AI_SYSTEM,
            serde_json::json!(GCP_VERTEX_AGENT_SYSTEM),
        )]);
        assert!(select_genai_vendor_fallback(&span).is_some());
    }

    #[test]
    fn test_select_vendor_fallback_via_prefix() {
        let span = make_span(vec![(
            GCP_VERTEX_SESSION_ID,
            serde_json::json!("session-1"),
        )]);
        assert!(select_genai_vendor_fallback(&span).is_some());
    }

    #[test]
    fn test_select_vendor_fallback_returns_none_for_other_systems() {
        let span = make_span(vec![(GEN_AI_SYSTEM, serde_json::json!("openai"))]);
        assert!(select_genai_vendor_fallback(&span).is_none());
    }

    #[test]
    fn test_vendor_fallback_does_not_overwrite_standard() {
        let span = make_span(vec![
            (GEN_AI_OPERATION_NAME, serde_json::json!("generate_content")),
            (GEN_AI_REQUEST_MODEL, serde_json::json!("standard-model")),
            (GEN_AI_USAGE_INPUT_TOKENS, serde_json::json!(5)),
            (GEN_AI_USAGE_OUTPUT_TOKENS, serde_json::json!(6)),
            (
                GEN_AI_RESPONSE_FINISH_REASONS,
                serde_json::json!(["standard-stop"]),
            ),
            (
                GCP_VERTEX_LLM_REQUEST,
                serde_json::json!({"model": "vendor-model"}),
            ),
            (
                GCP_VERTEX_LLM_RESPONSE,
                serde_json::json!({
                    "usage_metadata": {
                        "prompt_token_count": 50,
                        "candidates_token_count": 60
                    },
                    "finish_reason": "vendor-stop"
                }),
            ),
        ]);

        let result = extract_gen_ai_span(&span).expect("should extract");
        assert_eq!(result.request_model.as_deref(), Some("standard-model"));
        assert_eq!(result.input_tokens, Some(5));
        assert_eq!(result.output_tokens, Some(6));
        assert_eq!(result.finish_reasons, vec!["standard-stop"]);
    }

    #[test]
    fn test_event_content_wins_over_vendor() {
        let event_content = r#"[{"role":"user","content":"event"}]"#;
        let event = make_event(
            GEN_AI_EVENT_INFERENCE_DETAILS,
            vec![(GEN_AI_INPUT_MESSAGES, serde_json::json!(event_content))],
        );
        let span = make_span_with_events(
            vec![
                (GEN_AI_OPERATION_NAME, serde_json::json!("generate_content")),
                (
                    GCP_VERTEX_LLM_REQUEST,
                    serde_json::json!({"contents": [{"role": "user", "content": "vendor"}]}),
                ),
            ],
            vec![event],
        );

        let result = extract_gen_ai_span(&span).expect("should extract");
        assert_eq!(result.input_messages.as_deref(), Some(event_content));
    }

    #[test]
    fn test_vendor_session_id_to_conversation_id() {
        let span = make_span(vec![
            (GEN_AI_OPERATION_NAME, serde_json::json!("generate_content")),
            (GCP_VERTEX_SESSION_ID, serde_json::json!("session-123")),
        ]);

        let result = extract_gen_ai_span(&span).expect("should extract");
        assert_eq!(result.conversation_id.as_deref(), Some("session-123"));
    }

    #[test]
    fn test_vendor_llm_request_subfields() {
        let span = make_span(vec![
            (GEN_AI_OPERATION_NAME, serde_json::json!("generate_content")),
            (
                GCP_VERTEX_LLM_REQUEST,
                serde_json::json!({
                    "model": "gemini-2.5-pro",
                    "contents": [{"role": "user", "parts": [{"text": "hi"}]}],
                    "config": {
                        "system_instruction": {"parts": [{"text": "be concise"}]},
                        "tools": [{"function_declarations": [{"name": "search"}]}],
                        "top_p": 0.8,
                        "max_output_tokens": 1024
                    }
                }),
            ),
        ]);

        let result = extract_gen_ai_span(&span).expect("should extract");
        assert_eq!(result.request_model.as_deref(), Some("gemini-2.5-pro"));
        assert!(result.input_messages.as_deref().unwrap().contains("user"));
        assert!(
            result
                .system_instructions
                .as_deref()
                .unwrap()
                .contains("be concise")
        );
        assert!(
            result
                .tool_definitions
                .as_deref()
                .unwrap()
                .contains("search")
        );
        assert_eq!(result.request_top_p, Some(0.8));
        assert_eq!(result.request_max_tokens, Some(1024));
    }

    #[test]
    fn test_vendor_llm_response_subfields() {
        let span = make_span(vec![
            (GEN_AI_OPERATION_NAME, serde_json::json!("generate_content")),
            (
                GCP_VERTEX_LLM_RESPONSE,
                serde_json::json!({
                    "content": {"role": "model", "parts": [{"text": "hello"}]},
                    "usage_metadata": {
                        "prompt_token_count": 12,
                        "candidates_token_count": 24,
                        "thoughts_token_count": 7
                    },
                    "finish_reason": ["STOP", "SAFETY"]
                }),
            ),
        ]);

        let result = extract_gen_ai_span(&span).expect("should extract");
        assert!(result.output_messages.as_deref().unwrap().contains("hello"));
        assert_eq!(result.input_tokens, Some(12));
        assert_eq!(result.output_tokens, Some(24));
        assert_eq!(result.reasoning_output_tokens, Some(7));
        assert_eq!(result.finish_reasons, vec!["STOP", "SAFETY"]);
    }

    #[test]
    fn test_vendor_thoughts_token_to_reasoning_output_tokens() {
        let span = make_span(vec![
            (GEN_AI_OPERATION_NAME, serde_json::json!("generate_content")),
            (
                GCP_VERTEX_LLM_RESPONSE,
                serde_json::json!({"usage_metadata": {"thoughts_token_count": 9}}),
            ),
        ]);

        let result = extract_gen_ai_span(&span).expect("should extract");
        assert_eq!(result.reasoning_output_tokens, Some(9));
    }

    #[test]
    fn test_malformed_vendor_json_is_ignored() {
        let span = make_span(vec![
            (GEN_AI_OPERATION_NAME, serde_json::json!("generate_content")),
            (GCP_VERTEX_LLM_REQUEST, serde_json::json!("{not-json")),
        ]);

        let result = extract_gen_ai_span(&span).expect("span should still extract");
        assert!(result.request_model.is_none());
        assert!(result.input_messages.is_none());
    }

    #[test]
    fn test_raw_only_vendor_fields_not_structured() {
        let span = make_span(vec![
            (GEN_AI_OPERATION_NAME, serde_json::json!("generate_content")),
            (GEN_AI_SYSTEM, serde_json::json!(GCP_VERTEX_AGENT_SYSTEM)),
            (GCP_VERTEX_INVOCATION_ID, serde_json::json!("invoke-1")),
            (GCP_VERTEX_EVENT_ID, serde_json::json!("event-1")),
            (GCP_VERTEX_DATA, serde_json::json!({"raw": true})),
            (GCP_MCP_SERVER_DESTINATION_ID, serde_json::json!("mcp-1")),
        ]);

        let result = extract_gen_ai_span(&span).expect("should extract");
        assert!(result.conversation_id.is_none());
        assert!(result.request_model.is_none());
        assert!(result.tool_call_arguments.is_none());
        assert!(result.tool_call_result.is_none());
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
