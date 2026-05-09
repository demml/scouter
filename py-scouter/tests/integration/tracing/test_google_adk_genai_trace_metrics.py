import json
import time
import uuid
from collections.abc import AsyncGenerator
from contextlib import suppress
from typing import Any

import pytest
from google.adk.agents import Agent
from google.adk.models.base_llm import BaseLlm
from google.adk.models.llm_request import LlmRequest
from google.adk.models.llm_response import LlmResponse
from google.adk.runners import Runner
from google.adk.sessions import InMemorySessionService
from google.genai import types
from scouter.mock import ScouterDataFrameTestServer
from scouter.tracing import BatchConfig, ScouterInstrumentor, shutdown_tracer
from scouter.transport import GrpcConfig
from tests.integration.tracing.adk_helpers import refresh_google_adk_cached_tracers

POLL_TIMEOUT_SECS = 30
POLL_INTERVAL_SECS = 1
MODEL_NAME = "gemini-2.0-flash"
AGENT_NAME = "google_interactive_agent"
TOOL_NAME = "dinner_lookup"


def dinner_lookup(ingredient: str) -> dict[str, str]:
    """Return a deterministic dinner suggestion for an ingredient."""
    return {
        "ingredient": ingredient,
        "dish": "tofu, broccoli, and rice with a soy ginger glaze",
    }


def _cleanup_tracing_state() -> None:
    if ScouterInstrumentor._provider is not None or ScouterInstrumentor._instance is not None:
        with suppress(Exception):
            ScouterInstrumentor().uninstrument()

    with suppress(Exception):
        shutdown_tracer()

    ScouterInstrumentor._instance = None
    ScouterInstrumentor._provider = None


@pytest.fixture(autouse=True)
def reset_scouter_tracing_state():
    _cleanup_tracing_state()
    yield
    _cleanup_tracing_state()


class DeterministicGoogleLlm(BaseLlm):
    async def generate_content_async(
        self,
        llm_request: LlmRequest,
        stream: bool = False,
    ) -> AsyncGenerator[LlmResponse, None]:
        assert stream is False
        calls = getattr(self, "_calls", 0)
        object.__setattr__(self, "_calls", calls + 1)
        if calls == 0:
            yield LlmResponse(
                model_version=llm_request.model,
                content=types.Content(
                    role="model",
                    parts=[
                        types.Part.from_function_call(
                            name=TOOL_NAME,
                            args={"ingredient": "tofu"},
                        )
                    ],
                ),
                partial=False,
                finish_reason=types.FinishReason.STOP,
            )
            return

        yield LlmResponse(
            model_version=llm_request.model,
            content=types.Content(
                role="model",
                parts=[types.Part(text="Use tofu, broccoli, and rice with a soy ginger glaze.")],
            ),
            partial=False,
            finish_reason=types.FinishReason.STOP,
            usage_metadata=types.GenerateContentResponseUsageMetadata(
                prompt_token_count=21,
                candidates_token_count=13,
                total_token_count=34,
            ),
        )


class DeterministicGoogleAgent(Agent):
    def __init__(self) -> None:
        super().__init__(
            model=DeterministicGoogleLlm(model=MODEL_NAME),
            name=AGENT_NAME,
            description="Interactive assistant",
            instruction="Answer the user's cooking question with one concise sentence.",
            tools=[dinner_lookup],
        )


async def _run_google_adk_agent() -> str:
    agent = DeterministicGoogleAgent()
    session_service = InMemorySessionService()
    runner = Runner(
        agent=agent,
        app_name="scouter_google_interactive",
        session_service=session_service,
    )
    session = await session_service.create_session(
        app_name="scouter_google_interactive",
        user_id="evaluate_user",
    )
    message = types.Content(
        role="user",
        parts=[types.Part(text="Give me a quick tofu dinner idea.")],
    )
    response = ""

    async for event in runner.run_async(
        user_id="evaluate_user",
        session_id=session.id,
        new_message=message,
    ):
        if event.is_final_response() and event.content and event.content.parts:
            for part in event.content.parts:
                if part.text:
                    response = part.text

    if not response:
        raise AssertionError("Google ADK runner did not produce a final text response")
    return response


def _fetch_genai_spans(
    server: ScouterDataFrameTestServer,
) -> list[dict[str, Any]]:
    payload = server.genai_spans_json(
        limit=50,
    )
    return json.loads(payload)["spans"]


def _wait_for_adk_genai_spans(
    server: ScouterDataFrameTestServer,
) -> list[dict[str, Any]]:
    deadline = time.monotonic() + POLL_TIMEOUT_SECS
    last_spans: list[dict[str, Any]] = []

    while time.monotonic() < deadline:
        last_spans = _fetch_genai_spans(server)
        operations = {span["operation_name"] for span in last_spans}
        if {"invoke_agent", "generate_content", "execute_tool"}.issubset(operations):
            return last_spans
        time.sleep(POLL_INTERVAL_SECS)

    raise AssertionError(f"Timed out waiting for ADK GenAI spans: {last_spans}")


def _span_with_operation(
    spans: list[dict[str, Any]],
    operation_name: str,
) -> dict[str, Any]:
    for span in spans:
        if span["operation_name"] == operation_name:
            return span
    raise AssertionError(f"Missing GenAI span for operation: {operation_name}")


@pytest.mark.asyncio
async def test_google_adk_agent_writes_parseable_genai_trace_metrics() -> None:
    service_name = f"google-adk-trace-{uuid.uuid4().hex[:8]}"

    with ScouterDataFrameTestServer() as server:
        instrumentor = ScouterInstrumentor()
        instrumentor.instrument(
            transport_config=GrpcConfig(),
            batch_config=BatchConfig(scheduled_delay_ms=200),
            attributes={"service.name": service_name},
        )
        refresh_google_adk_cached_tracers()

        try:
            response = await _run_google_adk_agent()

            assert "tofu" in response.lower()

            spans = _wait_for_adk_genai_spans(
                server,
            )

            invoke_span = _span_with_operation(spans, "invoke_agent")
            generate_span = _span_with_operation(spans, "generate_content")
            tool_span = _span_with_operation(spans, "execute_tool")
            trace_id = generate_span["trace_id"]

            assert invoke_span["trace_id"] == trace_id
            assert invoke_span["agent_name"] == AGENT_NAME
            assert invoke_span["agent_description"] == "Interactive assistant"
            assert invoke_span["conversation_id"]

            assert generate_span["request_model"] == MODEL_NAME
            assert generate_span["provider_name"] == "gemini"
            assert generate_span["agent_name"] == AGENT_NAME
            assert generate_span["conversation_id"] == invoke_span["conversation_id"]
            assert generate_span["input_tokens"] == 21
            assert generate_span["output_tokens"] == 13
            assert generate_span["finish_reasons"] == ["stop"]

            assert tool_span["trace_id"] == trace_id
            assert tool_span["tool_call_arguments"]
            assert tool_span["tool_call_result"]
            assert json.loads(tool_span["tool_call_arguments"])["ingredient"] == "tofu"
            assert json.loads(tool_span["tool_call_result"])["dish"].startswith("tofu")

            metrics = json.loads(
                server.genai_trace_metrics_json(
                    trace_id=trace_id,
                    limit=25,
                )
            )

            assert metrics["trace_id"] == trace_id
            assert metrics["has_genai_spans"] is True
            assert metrics["sensitive_content_redacted"] is False
            assert {span["operation_name"] for span in metrics["spans"]} >= {
                "invoke_agent",
                "generate_content",
            }

            token_bucket = metrics["token_metrics"]["buckets"][0]
            assert token_bucket["total_input_tokens"] == 21
            assert token_bucket["total_output_tokens"] == 13

            operations = {operation["operation_name"] for operation in metrics["operation_breakdown"]["operations"]}
            assert "generate_content" in operations

            models = {model["model"] for model in metrics["model_usage"]["models"]}
            assert MODEL_NAME in models

            agents = {activity["agent_name"] for activity in metrics["agent_activity"]["agents"]}
            assert AGENT_NAME in agents
        finally:
            instrumentor.uninstrument()


def test_adk_raw_vendor_attrs_preserved() -> None:
    pytest.skip(
        "ScouterDataFrameTestServer exposes canonical GenAI spans only; raw trace attributes "
        "remain stored on TraceSpanRecord and are covered by Rust extraction tests."
    )
