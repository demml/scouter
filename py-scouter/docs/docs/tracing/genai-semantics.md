# GenAI semantic conventions

OpenTelemetry defines [GenAI semantic conventions](https://opentelemetry.io/docs/specs/semconv/gen-ai/) across spans and events. Scouter extracts both: span attributes map to structured fields on `GenAiSpanRecord`, and two specific event types (`gen_ai.client.inference.operation.details` and `gen_ai.evaluation.result`) populate opt-in content and evaluation results.

Metrics like token usage and operation latency are computed from span attributes via DataFusion queries. You don't need a separate OTel metrics pipeline.

---

## What triggers GenAI extraction

A span is recognized as a GenAI span if and only if it has the `gen_ai.operation.name` attribute set. Scouter scans all attributes once on ingest and populates a `GenAiSpanRecord` from them. Spans without this attribute go through the normal span path and are unaffected.

This mirrors the OTel spec, where `gen_ai.operation.name` is the required attribute for all GenAI spans.

---

## Supported span attributes

| OTel attribute | Field in `GenAiSpanRecord` | Notes |
|---|---|---|
| `gen_ai.operation.name` | `operation_name` | Required — absence skips extraction |
| `gen_ai.provider.name` | `provider_name` | e.g. `openai`, `anthropic`, `google` |
| `gen_ai.request.model` | `request_model` | Model the caller requested |
| `gen_ai.response.model` | `response_model` | Model that served the response (may differ) |
| `gen_ai.response.id` | `response_id` | Provider-assigned response ID |
| `gen_ai.usage.input_tokens` | `input_tokens` | Prompt tokens |
| `gen_ai.usage.output_tokens` | `output_tokens` | Completion tokens |
| `gen_ai.usage.cache_creation.input_tokens` | `cache_creation_input_tokens` | Anthropic cache write tokens |
| `gen_ai.usage.cache_read.input_tokens` | `cache_read_input_tokens` | Anthropic cache read tokens |
| `gen_ai.response.finish_reasons` | `finish_reasons` | JSON array, JSON-encoded array string, or bare string |
| `gen_ai.output.type` | `output_type` | e.g. `text`, `json`, `image` |
| `gen_ai.conversation.id` | `conversation_id` | Groups multi-turn spans into a conversation |
| `gen_ai.agent.name` | `agent_name` | Agent name for multi-agent systems |
| `gen_ai.agent.id` | `agent_id` | |
| `gen_ai.agent.description` | `agent_description` | Free-form agent description |
| `gen_ai.agent.version` | `agent_version` | e.g. `1.0.0`, `2025-05-01` |
| `gen_ai.data_source.id` | `data_source_id` | RAG / retrieval data source identifier |
| `gen_ai.tool.name` | `tool_name` | Tool invoked in a tool-use span |
| `gen_ai.tool.type` | `tool_type` | e.g. `function`, `retrieval` |
| `gen_ai.tool.call.id` | `tool_call_id` | Provider-assigned call ID |
| `gen_ai.request.temperature` | `request_temperature` | |
| `gen_ai.request.max_tokens` | `request_max_tokens` | |
| `gen_ai.request.top_p` | `request_top_p` | |
| `gen_ai.request.choice.count` | `request_choice_count` | Number of candidate completions requested |
| `gen_ai.request.seed` | `request_seed` | |
| `gen_ai.request.frequency_penalty` | `request_frequency_penalty` | |
| `gen_ai.request.presence_penalty` | `request_presence_penalty` | |
| `gen_ai.request.stop_sequences` | `request_stop_sequences` | String array; accepts JSON-encoded array or raw array value |
| `server.address` | `server_address` | GenAI server hostname or IP |
| `server.port` | `server_port` | Required when `server.address` is set |
| `error.type` | `error_type` | Error classification on failed spans |
| `openai.api.type` | `openai_api_type` | OpenAI-specific: `chat`, `responses`, etc. |
| `openai.response.service_tier` | `openai_service_tier` | e.g. `default`, `flex` |
| `gen_ai.input.messages` | `input_messages` | Opt-in; see [Opt-in content](#opt-in-content) |
| `gen_ai.output.messages` | `output_messages` | Opt-in; see [Opt-in content](#opt-in-content) |
| `gen_ai.system_instructions` | `system_instructions` | Opt-in; see [Opt-in content](#opt-in-content) |
| `gen_ai.tool.definitions` | `tool_definitions` | Opt-in; see [Opt-in content](#opt-in-content) |

Token fields accept JSON numbers, floats, or numeric strings. `finish_reasons` handles all three of: a JSON array, a JSON-encoded array string, and a bare string.

---

## Span events

Scouter recognizes two OTel GenAI event types and extracts their content into `GenAiSpanRecord`. Everything else passes through to `TraceSpanRecord.events` unchanged.

### gen_ai.client.inference.operation.details

This event duplicates the parent span's scalar attributes (tokens, model, operation name, finish reasons) and adds the four opt-in content fields. The duplication is intentional: the event is designed to work standalone, without a span context attached.

When the event is on a span, Scouter ignores the scalar attributes from the event entirely. Span attributes win for scalars, full stop. Only the four opt-in content fields (`input_messages`, `output_messages`, `system_instructions`, `tool_definitions`) use event fallback: if missing from the span, Scouter reads them from the event instead.

### gen_ai.evaluation.result

No span attribute equivalent exists for eval scores. They always come from events. A span can have multiple `gen_ai.evaluation.result` events attached; Scouter collects all of them into `eval_results`. Events without `gen_ai.evaluation.name` are silently dropped — it's the only required field.

### Source-of-truth rule

```
Scalar attrs (tokens, model, operation, finish_reasons, etc.)
  → span attributes only, no event fallback

Opt-in content (input_messages, output_messages, system_instructions, tool_definitions)
  → span attribute first; falls back to gen_ai.client.inference.operation.details event

gen_ai.evaluation.result
  → events only, no span attribute equivalent
```

Token counts, latency, model breakdown, agent activity: all computed from span attributes. Event processing doesn't affect them.

---

## Opt-in content

The four opt-in content fields (`input_messages`, `output_messages`, `system_instructions`, `tool_definitions`) are off by default. Your instrumentation library has to explicitly enable them. On spans they're JSON-encoded strings; on the `gen_ai.client.inference.operation.details` event they can be structured objects.

Don't turn them on without a reason. A multi-turn conversation with tool call results can be hundreds of kilobytes per span, and that adds up fast under load. For most observability needs — cost tracking, latency analysis, error rates — the scalar fields are all you need. Turn the content fields on when you actually need the message bodies: building evaluation datasets from production traces, debugging prompt regressions, conversation replay.

```python
from scouter.tracing import get_tracer
import json

tracer = get_tracer("my-llm-service")

with tracer.start_as_current_span("llm_call") as span:
    span.set_attribute("gen_ai.operation.name", "chat")
    span.set_attribute("gen_ai.provider.name", "anthropic")
    span.set_attribute("gen_ai.request.model", "claude-opus-4-6")
    span.set_attribute("gen_ai.usage.input_tokens", 512)
    span.set_attribute("gen_ai.usage.output_tokens", 128)

    # Opt-in content — only set if you need it
    span.set_attribute("gen_ai.input.messages", json.dumps([
        {"role": "user", "parts": [{"type": "text", "content": "What is RAG?"}]}
    ]))

    response = call_my_llm(prompt)

    span.set_attribute("gen_ai.output.messages", json.dumps([
        {"role": "assistant", "parts": [{"type": "text", "content": response.text}],
         "finish_reason": "end_turn"}
    ]))
```

---

## Evaluation results

Eval scores attach to spans as `gen_ai.evaluation.result` events. Each event maps to a `GenAiEvalResult` entry in `eval_results`:

| Field | Type | OTel attribute |
|---|---|---|
| `name` | `str` | `gen_ai.evaluation.name` (required) |
| `score_label` | `str \| None` | `gen_ai.evaluation.score.label` |
| `score_value` | `float \| None` | `gen_ai.evaluation.score.value` |
| `explanation` | `str \| None` | `gen_ai.evaluation.explanation` |
| `response_id` | `str \| None` | `gen_ai.response.id` |

A span can carry multiple eval events, one per metric. Scouter collects all of them into `eval_results`. If your instrumentation library emits these events, they're captured automatically. You can also add them manually:

```python
from opentelemetry import trace

span = trace.get_current_span()
span.add_event("gen_ai.evaluation.result", {
    "gen_ai.evaluation.name": "Relevance",
    "gen_ai.evaluation.score.value": 0.92,
    "gen_ai.evaluation.score.label": "relevant",
    "gen_ai.evaluation.explanation": "The response directly addresses the user question.",
    "gen_ai.response.id": "chatcmpl-abc123",
})
```

An event missing `gen_ai.evaluation.name` is silently skipped. If a span has no eval events, `eval_results` is an empty list.

---

## Automatic capture with framework integrations

Frameworks that emit GenAI semantic convention attributes work without extra configuration. Register `ScouterInstrumentor` before any framework code runs.

=== "OpenAI Agents SDK"

    ```python
    from scouter.tracing import ScouterInstrumentor
    from scouter import GrpcConfig
    from agents import Agent, Runner

    ScouterInstrumentor().instrument(
        transport_config=GrpcConfig(),
        attributes={"service.name": "openai-agent-service"},
    )

    agent = Agent(name="ResearchAgent", instructions="...", tools=[...])
    result = Runner.run_sync(agent, "Summarize recent advances in RAG.")
    # LLM call spans carry gen_ai.* attributes — extracted automatically
    ```

=== "Google ADK"

    ```python
    from scouter.tracing import ScouterInstrumentor
    from scouter.transport import GrpcConfig

    # ADK calls get_tracer_provider() internally — picks up Scouter automatically
    ScouterInstrumentor().instrument(
        transport_config=GrpcConfig(),
        attributes={"service.name": "adk-service"},
    )
    ```

=== "LangChain"

    ```python
    from scouter.tracing import ScouterInstrumentor
    from scouter import GrpcConfig
    from opentelemetry.instrumentation.langchain import LangchainInstrumentor

    ScouterInstrumentor().instrument(transport_config=GrpcConfig())
    LangchainInstrumentor().instrument()
    ```

See the [ScouterInstrumentor guide](../tracing/instrumentor.md) for full setup and framework coverage.

---

## Adding attributes manually

If your code calls an LLM directly without a pre-instrumented SDK, set the attributes yourself. The only hard requirement is `gen_ai.operation.name`.

```python
from scouter.tracing import get_tracer

tracer = get_tracer("my-llm-service")

with tracer.start_as_current_span("llm_call") as span:
    span.set_attribute("gen_ai.operation.name", "chat")
    span.set_attribute("gen_ai.provider.name", "anthropic")
    span.set_attribute("gen_ai.request.model", "claude-opus-4-6")
    span.set_attribute("gen_ai.request.temperature", 0.7)
    span.set_attribute("gen_ai.request.max_tokens", 1024)
    span.set_attribute("gen_ai.usage.input_tokens", 512)
    span.set_attribute("gen_ai.usage.output_tokens", 128)
    span.set_attribute("gen_ai.response.finish_reasons", '["end_turn"]')
    span.set_attribute("server.address", "api.anthropic.com")
    span.set_attribute("server.port", 443)

    response = call_my_llm(prompt)
    span.set_output({"text": response.content})
```

For multi-turn conversations, set `gen_ai.conversation.id` consistently across all spans in the session:

```python
span.set_attribute("gen_ai.conversation.id", session_id)
```

For agent spans, include the agent metadata so the agent activity endpoint can aggregate correctly:

```python
span.set_attribute("gen_ai.operation.name", "invoke_agent")
span.set_attribute("gen_ai.agent.name", "ResearchAgent")
span.set_attribute("gen_ai.agent.id", "asst_abc123")
span.set_attribute("gen_ai.agent.description", "Searches and summarizes research papers.")
span.set_attribute("gen_ai.agent.version", "1.2.0")
```

For RAG applications that use a data source:

```python
span.set_attribute("gen_ai.data_source.id", "H7STPQYOND")
```

---

## Querying GenAI data

The server exposes read endpoints under `/scouter/genai/`. Metrics endpoints accept `POST` with a `GenAiMetricsRequest` body; `service_name`, `operation_name`, `provider_name`, and `model` are optional filters.

```json
{
  "start_time":      "2026-04-01T00:00:00Z",
  "end_time":        "2026-04-19T23:59:59Z",
  "service_name":    "my-llm-service",
  "operation_name":  "chat",
  "provider_name":   "anthropic",
  "model":           "claude-opus-4-6",
  "bucket_interval": "hour"
}
```

### Token metrics

`POST /scouter/genai/metrics/tokens`

Input/output token counts bucketed by time, with error rate per bucket. `bucket_interval` defaults to `"hour"`.

```python
import httpx

client = httpx.Client(base_url="http://scouter:8000")

resp = client.post("/scouter/genai/metrics/tokens", json={
    "start_time": "2026-04-18T00:00:00Z",
    "end_time":   "2026-04-19T00:00:00Z",
    "service_name": "my-llm-service",
})

for bucket in resp.json()["buckets"]:
    print(bucket["bucket_start"], bucket["total_input_tokens"], bucket["total_output_tokens"])
```

Response fields per bucket: `bucket_start`, `total_input_tokens`, `total_output_tokens`, `total_cache_creation_tokens`, `total_cache_read_tokens`, `span_count`, `error_rate`.

### Operation breakdown

`POST /scouter/genai/metrics/operations`

Per-operation aggregates: span count, average latency, total tokens, error rate. Useful for understanding cost split between chat, embeddings, and tool calls.

Response fields per operation: `operation_name`, `provider_name`, `span_count`, `avg_duration_ms`, `total_input_tokens`, `total_output_tokens`, `error_rate`.

### Model usage

`POST /scouter/genai/metrics/models`

Per-model aggregates with p50/p95 latency percentiles. Compare performance across model versions or providers.

Response fields per model: `model`, `provider_name`, `span_count`, `total_input_tokens`, `total_output_tokens`, `p50_duration_ms`, `p95_duration_ms`, `error_rate`.

### Agent activity

`POST /scouter/genai/metrics/agents`

Per-agent aggregates. Accepts an optional `agent_name` query parameter.

```python
resp = client.post(
    "/scouter/genai/metrics/agents",
    params={"agent_name": "ResearchAgent"},
    json={"start_time": "2026-04-18T00:00:00Z", "end_time": "2026-04-19T00:00:00Z"},
)
```

Response fields per agent: `agent_name`, `agent_id`, `conversation_id`, `span_count`, `total_input_tokens`, `total_output_tokens`, `last_seen`.

### Tool activity

`POST /scouter/genai/metrics/tools`

Per-tool aggregates: call count, average duration, error rate.

Response fields: `tool_name`, `tool_type`, `call_count`, `avg_duration_ms`, `error_rate`.

### Error breakdown

`POST /scouter/genai/metrics/errors`

Counts by `error.type` value for the given time window.

```python
resp = client.post("/scouter/genai/metrics/errors", json={
    "start_time": "2026-04-18T00:00:00Z",
    "end_time":   "2026-04-19T00:00:00Z",
})
for err in resp.json()["errors"]:
    print(err["error_type"], err["count"])
```

### Raw GenAI spans

`POST /scouter/genai/spans`

Returns individual `GenAiSpanRecord` objects matching a filter set. Each record includes all extracted span attributes plus `eval_results` (a list of `GenAiEvalResult` objects, empty if no eval events were present) and the four opt-in content fields (`input_messages`, `output_messages`, `system_instructions`, `tool_definitions`) when they were recorded.

```python
resp = client.post("/scouter/genai/spans", json={
    "service_name":   "my-llm-service",
    "provider_name":  "anthropic",
    "start_time":     "2026-04-18T00:00:00Z",
    "end_time":       "2026-04-19T00:00:00Z",
    "limit":          100,
})
```

Available filters: `service_name`, `start_time`, `end_time`, `operation_name`, `provider_name`, `model`, `conversation_id`, `agent_name`, `tool_name`, `error_type`, `limit`.

### Conversation spans

`GET /scouter/genai/conversation/{conversation_id}`

Returns all GenAI spans for a conversation, ordered chronologically. Accepts optional `start_time` and `end_time` query parameters in RFC3339 format.

```python
resp = client.get(
    f"/scouter/genai/conversation/{conversation_id}",
    params={"start_time": "2026-04-18T00:00:00Z", "end_time": "2026-04-19T00:00:00Z"},
)
```

`conversation_id` longer than 256 characters returns `400`.

---

## OpsML integration

Scouter's GenAI span data is consumed by [OpsML](https://github.com/demml/opsml), which provides a frontend for browsing token usage trends, model latency, agent activity, and conversation traces. If you're running OpsML alongside Scouter, the GenAI views are populated automatically once spans with `gen_ai.operation.name` start flowing in.

---

## TraceAssertionTask

GenAI spans stored in Scouter are queryable by `TraceAssertionTask` during offline or online evaluation — useful for writing assertions against token budgets, latency SLAs, or model behavior across production traces. See [TraceAssertionTask](/scouter/docs/monitoring/genai/tasks/#traceassertionask) for details.
