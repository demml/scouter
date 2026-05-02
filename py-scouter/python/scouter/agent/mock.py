"""ScouterMockAgent — deterministic mock that emits real OTel genai spans.

Designed to exercise the full eval harness without calling a live LLM.

The agent emits spans via ``opentelemetry.trace`` exactly as a real 3rd-party
framework would (google-adk, openai-agents, etc). ``ScouterInstrumentor``
intercepts those spans through its OTel exporter hook.

Lifecycle callbacks mirror the google-adk pattern so mock examples and real
examples share the same structural shape:

    after_model_callback(query, response)  — emits EvalRecord, records metrics, etc.
    before_model_callback(query)           — inject headers, log, gate the call, etc.
    after_tool_callback(tool_name, result) — validate tool output, log, etc.

Span tree emitted per ``run()`` call:
    invoke_agent <alias>              (gen_ai.operation.name=invoke_agent)
      call_llm                        (gen_ai.operation.name=chat, token counts)
        generate_content <model>      (gen_ai.response.model, token counts)
      execute_tool <tool_name>        (gen_ai.tool.name, per tool called)
"""

from __future__ import annotations

import uuid
from dataclasses import dataclass, field
from typing import Any, Callable

try:
    from opentelemetry import trace as _otel_trace

    _OTEL_AVAILABLE = True
except ImportError:  # pragma: no cover
    _OTEL_AVAILABLE = False


@dataclass
class MockTool:
    name: str
    fn: Callable[[str], str]
    description: str = ""


BeforeModelCallback = Callable[[str], None]
AfterModelCallback = Callable[[str, str], None]
AfterToolCallback = Callable[[str, str], None]


@dataclass
class ScouterMockAgent:
    """Deterministic mock agent that emits standard genai OTel spans.

    Args:
        alias: Agent alias (used in span name and forwarded to callbacks).
        model_name: Reported in ``gen_ai.request.model`` / ``gen_ai.response.model``.
        provider: Reported in ``gen_ai.system`` (e.g. ``"openai"``, ``"anthropic"``).
        tools: Tools the agent can invoke. Selected by keyword match against the query.
        response_fn: Override response generation. Receives the query, returns a string.
        before_model_callback: Called with ``(query)`` before the model span opens.
        after_model_callback: Called with ``(query, response)`` after the model span closes.
        after_tool_callback: Called with ``(tool_name, result)`` after each tool span closes.
    """

    alias: str
    model_name: str = "mock-model-1.0"
    provider: str = "mock"
    tools: list[MockTool] = field(default_factory=list)
    response_fn: Callable[[str], str] | None = None
    before_model_callback: BeforeModelCallback | None = None
    after_model_callback: AfterModelCallback | None = None
    after_tool_callback: AfterToolCallback | None = None

    def _generate_response(self, query: str) -> str:
        if self.response_fn is not None:
            return self.response_fn(query)
        return (
            f"Step 1: Analyze '{query[:40]}'. "
            "Step 2: Apply best practices. "
            "Step 3: Verify the outcome. "
            "Risk: edge cases may require iteration. DONE"
        )

    def _select_tools(self, query: str) -> list[MockTool]:
        lowered = query.lower()
        matched = [t for t in self.tools if t.name.lower() in lowered]
        return matched or self.tools[:1]

    def run(self, query: str) -> str:
        if not _OTEL_AVAILABLE:
            raise RuntimeError(
                "opentelemetry-api is required for ScouterMockAgent. "
                + "Install it with: pip install opentelemetry-api"
            )
        response = self._generate_response(query)
        input_tokens = max(1, len(query.split()))
        output_tokens = max(1, len(response.split()))
        tracer = _otel_trace.get_tracer("mock.framework")
        self._emit_spans(tracer, query, response, input_tokens, output_tokens)
        return response

    def _emit_spans(
        self,
        tracer: Any,
        query: str,
        response: str,
        input_tokens: int,
        output_tokens: int,
    ) -> None:
        with tracer.start_as_current_span(f"invoke_agent {self.alias}") as agent_span:
            agent_span.set_attribute("gen_ai.agent.name", self.alias)
            agent_span.set_attribute("gen_ai.operation.name", "invoke_agent")
            agent_span.set_attribute("gen_ai.system", self.provider)

            if self.before_model_callback is not None:
                self.before_model_callback(query)

            with tracer.start_as_current_span("call_llm") as llm_span:
                llm_span.set_attribute("gen_ai.operation.name", "chat")
                llm_span.set_attribute("gen_ai.system", self.provider)
                llm_span.set_attribute("gen_ai.request.model", self.model_name)
                llm_span.set_attribute("gen_ai.usage.input_tokens", input_tokens)
                llm_span.set_attribute("gen_ai.usage.output_tokens", output_tokens)
                llm_span.set_attribute("gen_ai.response.finish_reasons", ["stop"])
                llm_span.set_attribute("gen_ai.response.id", uuid.uuid4().hex)

                with tracer.start_as_current_span(f"generate_content {self.model_name}") as gen_span:
                    gen_span.set_attribute("gen_ai.operation.name", "chat")
                    gen_span.set_attribute("gen_ai.system", self.provider)
                    gen_span.set_attribute("gen_ai.request.model", self.model_name)
                    gen_span.set_attribute("gen_ai.response.model", self.model_name)
                    gen_span.set_attribute("gen_ai.usage.input_tokens", input_tokens)
                    gen_span.set_attribute("gen_ai.usage.output_tokens", output_tokens)

            for tool in self._select_tools(query):
                with tracer.start_as_current_span(f"execute_tool {tool.name}") as tool_span:
                    tool_span.set_attribute("gen_ai.operation.name", "execute_tool")
                    tool_span.set_attribute("gen_ai.tool.name", tool.name)
                    tool_span.set_attribute("gen_ai.tool.type", "function")
                    tool_span.set_attribute("gen_ai.tool.call.id", uuid.uuid4().hex)
                    result = tool.fn(query)
                    if self.after_tool_callback is not None:
                        self.after_tool_callback(tool.name, result)

            if self.after_model_callback is not None:
                self.after_model_callback(query, response)
