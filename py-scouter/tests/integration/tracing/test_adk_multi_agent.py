from __future__ import annotations

from collections.abc import AsyncGenerator, Generator
from contextlib import suppress
from typing import Any

import pytest
from google.adk.agents import Agent, SequentialAgent  # noqa: E402
from google.adk.models.base_llm import BaseLlm  # noqa: E402
from google.adk.models.llm_request import LlmRequest  # noqa: E402
from google.adk.models.llm_response import LlmResponse  # noqa: E402
from google.adk.runners import Runner  # noqa: E402
from google.adk.sessions import InMemorySessionService  # noqa: E402
from google.genai import types  # noqa: E402
from scouter.drift import AgentEvalConfig, AgentEvalProfile
from scouter.evaluate import ComparisonOperator, TraceAssertion, TraceAssertionTask
from scouter.mock import MockConfig
from scouter.queue import ScouterQueue
from scouter.tracing import ScouterInstrumentor
from scouter.tracing import TestSpanExporter as _TestSpanExporter
from scouter.tracing import get_tracer, shutdown_tracer

MODEL_NAME = "gemini-2.0-flash"


def _cleanup_tracing_state() -> None:
    if ScouterInstrumentor._provider is not None or ScouterInstrumentor._instance is not None:
        with suppress(Exception):
            ScouterInstrumentor().uninstrument()

    with suppress(Exception):
        shutdown_tracer()

    ScouterInstrumentor._instance = None
    ScouterInstrumentor._provider = None


@pytest.fixture(autouse=True)
def reset_scouter_tracing_state() -> Generator[None, None, None]:
    _cleanup_tracing_state()
    yield
    _cleanup_tracing_state()


@pytest.fixture
def profile() -> AgentEvalProfile:
    return AgentEvalProfile(
        tasks=[
            TraceAssertionTask(
                id="trace_has_spans",
                assertion=TraceAssertion.trace_span_count(),
                operator=ComparisonOperator.GreaterThan,
                expected_value=0,
            )
        ],
        config=AgentEvalConfig(space="test", name="adk-trace-agent"),
        alias="adk-trace-agent",
    )


@pytest.fixture
def queue(profile: AgentEvalProfile) -> ScouterQueue:
    queue = ScouterQueue.from_profile(
        profile=[profile],
        transport_config=MockConfig(),
        wait_for_startup=True,
    )
    queue.enable_capture()
    return queue


class DeterministicGoogleLlm(BaseLlm):
    async def generate_content_async(
        self,
        llm_request: LlmRequest,
        stream: bool = False,
    ) -> AsyncGenerator[LlmResponse, None]:
        assert stream is False
        yield LlmResponse(
            model_version=llm_request.model,
            content=types.Content(
                role="model",
                parts=[types.Part(text=f"{llm_request.model} completed.")],
            ),
            partial=False,
            finish_reason=types.FinishReason.STOP,
            usage_metadata=types.GenerateContentResponseUsageMetadata(
                prompt_token_count=5,
                candidates_token_count=3,
                total_token_count=8,
            ),
        )


async def _run_sequential_agent(
    profile: AgentEvalProfile, queue: ScouterQueue
) -> tuple[str, dict[str, tuple[str, str]]]:
    instrumentor = ScouterInstrumentor()
    instrumentor.instrument(
        transport_config=MockConfig(),
        exporter=_TestSpanExporter(batch_export=False),
        scouter_queue=queue,
    )
    tracer = get_tracer("test.adk.attach_eval")
    callback_anchors: dict[str, tuple[str, str]] = {}

    def attach_callback(callback_context: Any) -> None:
        agent_name = callback_context.agent_name
        session_id = callback_context.session.id
        with tracer.start_as_current_span(f"adk.callback.{agent_name}") as span:
            span.attach_eval(
                profile_uid=profile.config.uid,
                context={
                    "agent_name": agent_name,
                    "invocation_id": callback_context.invocation_id,
                },
                record_id=f"{agent_name}:{callback_context.invocation_id}",
                session_id=session_id,
                tags=["source=google_adk", f"agent={agent_name}"],
            )
            callback_anchors[agent_name] = (span.trace_id, span.span_id)

    first = Agent(
        model=DeterministicGoogleLlm(model=MODEL_NAME),
        name="classifier_agent",
        description="Classifies the request",
        instruction="Classify the request.",
        after_agent_callback=attach_callback,
    )
    second = Agent(
        model=DeterministicGoogleLlm(model=MODEL_NAME),
        name="solution_agent",
        description="Answers the request",
        instruction="Answer the request.",
        after_agent_callback=attach_callback,
    )
    root = SequentialAgent(
        name="root_sequence",
        description="Runs classification then solution generation",
        sub_agents=[first, second],
    )

    session_service = InMemorySessionService()
    runner = Runner(
        agent=root,
        app_name="scouter_adk_attach_eval",
        session_service=session_service,
    )
    session = await session_service.create_session(
        app_name="scouter_adk_attach_eval",
        user_id="evaluate_user",
    )
    message = types.Content(
        role="user",
        parts=[types.Part(text="Help investigate a slow model response.")],
    )
    response = ""

    async for event in runner.run_async(
        user_id="evaluate_user",
        session_id=session.id,
        new_message=message,
    ):
        if event.is_final_response() and event.content and event.content.parts:
            response = "".join(part.text or "" for part in event.content.parts)

    return response, callback_anchors


@pytest.mark.asyncio
async def test_google_adk_sequential_agent_attach_eval_records_callback_anchors(
    profile: AgentEvalProfile,
    queue: ScouterQueue,
) -> None:
    response, callback_anchors = await _run_sequential_agent(profile, queue)

    assert "completed" in response

    records = queue.get_by_entity_uid(profile.config.uid).drain()
    assert len(records) == 2
    by_agent = {record.context["agent_name"]: record for record in records}

    assert set(by_agent) == {"classifier_agent", "solution_agent"}
    assert set(callback_anchors) == {"classifier_agent", "solution_agent"}
    assert by_agent["classifier_agent"].trace_id == callback_anchors["classifier_agent"][0]
    assert by_agent["classifier_agent"].span_id == callback_anchors["classifier_agent"][1]
    assert by_agent["solution_agent"].trace_id == callback_anchors["solution_agent"][0]
    assert by_agent["solution_agent"].span_id == callback_anchors["solution_agent"][1]
    assert by_agent["classifier_agent"].span_id != by_agent["solution_agent"].span_id
    assert by_agent["classifier_agent"].session_id == by_agent["solution_agent"].session_id
    assert "source=google_adk" in by_agent["classifier_agent"].tags
