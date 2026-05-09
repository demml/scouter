from __future__ import annotations

from collections.abc import Generator
from contextlib import suppress
from typing import Any

import pytest
from scouter.drift import AgentEvalConfig, AgentEvalProfile
from scouter.evaluate import (
    AssertionTask,
    ComparisonOperator,
    EvalRecord,
    ImageMedia,
    TraceAssertion,
    TraceAssertionTask,
)
from scouter.mock import MockConfig
from scouter.queue import ScouterQueue
from scouter.tracing import ScouterInstrumentor, StdoutSpanExporter
from scouter.tracing import TestSpanExporter as _TestSpanExporter
from scouter.tracing import get_tracer, shutdown_tracer


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
        config=AgentEvalConfig(space="test", name="trace-agent"),
        alias="trace-agent",
    )


@pytest.fixture
def content_only_profile() -> AgentEvalProfile:
    return AgentEvalProfile(
        tasks=[
            AssertionTask(
                id="response_is_string",
                context_path="response",
                operator=ComparisonOperator.IsString,
                expected_value=True,
            )
        ],
        config=AgentEvalConfig(space="test", name="content-agent"),
        alias="content-agent",
    )


@pytest.fixture
def queue(profile: AgentEvalProfile, content_only_profile: AgentEvalProfile) -> ScouterQueue:
    queue = ScouterQueue.from_profile(
        profile=[profile, content_only_profile],
        transport_config=MockConfig(),
        wait_for_startup=True,
    )
    queue.enable_capture()
    return queue


@pytest.fixture
def tracer(queue: ScouterQueue) -> Any:
    instrumentor = ScouterInstrumentor()
    instrumentor.instrument(
        transport_config=MockConfig(),
        exporter=_TestSpanExporter(batch_export=False),
        scouter_queue=queue,
    )
    return get_tracer("test.attach_eval")


def test_attach_eval_anchors_record_to_span(
    tracer: Any,
    queue: ScouterQueue,
    profile: AgentEvalProfile,
) -> None:
    with tracer.start_as_current_span("agent.callback") as span:
        span.attach_eval(
            profile_uid=profile.config.uid,
            context={"query": "plan dinner", "response": "tacos"},
        )
        captured_trace_id = span.trace_id
        captured_span_id = span.span_id

    records = queue.get_by_entity_uid(profile.config.uid).drain()
    assert len(records) == 1
    record = records[0]
    assert record.trace_id == captured_trace_id
    assert record.span_id == captured_span_id
    assert record.entity_uid == profile.config.uid
    assert record.context["query"] == "plan dinner"


def test_path_b_record_has_no_anchor(
    queue: ScouterQueue,
    content_only_profile: AgentEvalProfile,
) -> None:
    record = EvalRecord(
        profile_uid=content_only_profile.config.uid,
        context={"query": "weather", "response": "sunny"},
    )
    queue.get_by_entity_uid(content_only_profile.config.uid).insert(record)

    records = queue.get_by_entity_uid(content_only_profile.config.uid).drain()
    assert len(records) == 1
    assert records[0].trace_id is None
    assert records[0].span_id is None


def test_attach_eval_raises_without_queue(profile: AgentEvalProfile) -> None:
    instrumentor = ScouterInstrumentor()
    instrumentor.instrument(
        transport_config=MockConfig(),
        exporter=_TestSpanExporter(batch_export=False),
    )
    tracer = get_tracer("test.no_queue")

    with pytest.raises(RuntimeError, match="queue"):
        with tracer.start_as_current_span("orphan") as span:
            span.attach_eval(
                profile_uid=profile.config.uid,
                context={"query": "x"},
            )


def test_callback_records_anchor_to_callback_span_not_parent(
    tracer: Any,
    queue: ScouterQueue,
    profile: AgentEvalProfile,
) -> None:
    with tracer.start_as_current_span("runner"):
        pass

    with tracer.start_as_current_span("triage_callback") as triage:
        triage.attach_eval(
            profile_uid=profile.config.uid,
            context={"agent": "triage"},
        )
        triage_trace_id = triage.trace_id
        triage_span_id = triage.span_id

    with tracer.start_as_current_span("responder_callback") as responder:
        responder.attach_eval(
            profile_uid=profile.config.uid,
            context={"agent": "responder"},
        )
        responder_trace_id = responder.trace_id
        responder_span_id = responder.span_id

    records = queue.get_by_entity_uid(profile.config.uid).drain()
    assert len(records) == 2
    by_agent = {record.context["agent"]: record for record in records}

    assert by_agent["triage"].trace_id == triage_trace_id
    assert by_agent["triage"].span_id == triage_span_id
    assert by_agent["responder"].trace_id == responder_trace_id
    assert by_agent["responder"].span_id == responder_span_id
    assert by_agent["triage"].span_id != by_agent["responder"].span_id


def test_two_attach_eval_calls_share_anchor(
    tracer: Any,
    queue: ScouterQueue,
    profile: AgentEvalProfile,
) -> None:
    with tracer.start_as_current_span("multi_eval") as span:
        span.attach_eval(profile_uid=profile.config.uid, context={"step": 1})
        span.attach_eval(profile_uid=profile.config.uid, context={"step": 2})
        anchor_trace = span.trace_id
        anchor_span = span.span_id

    records = sorted(
        queue.get_by_entity_uid(profile.config.uid).drain(),
        key=lambda record: record.context["step"],
    )
    assert len(records) == 2
    assert records[0].trace_id == anchor_trace
    assert records[1].trace_id == anchor_trace
    assert records[0].span_id == anchor_span
    assert records[1].span_id == anchor_span
    assert records[0].uid != records[1].uid


def test_attach_eval_skips_unsampled_trace(
    queue: ScouterQueue,
    profile: AgentEvalProfile,
) -> None:
    instrumentor = ScouterInstrumentor()
    instrumentor.instrument(
        transport_config=MockConfig(),
        exporter=StdoutSpanExporter(batch_export=False),
        scouter_queue=queue,
        sample_ratio=0.0,
    )
    tracer = get_tracer("test.unsampled")

    with tracer.start_as_current_span("unsampled-trace") as span:
        span.attach_eval(profile_uid=profile.config.uid, context={"query": "x"})

    assert queue.get_by_entity_uid(profile.config.uid).drain() == []


def test_attach_eval_preserves_agent_workflow_metadata(
    tracer: Any,
    queue: ScouterQueue,
    profile: AgentEvalProfile,
) -> None:
    media = [
        ImageMedia(
            id="chart",
            url="https://example.com/chart.png",
            mime_type="image/png",
        )
    ]

    with tracer.start_as_current_span("agent.callback") as span:
        span.attach_eval(
            profile_uid=profile.config.uid,
            context={"query": "plan dinner", "response": "tacos"},
            record_id="turn-7",
            session_id="session-123",
            media=media,
            tags=["run_id=abc", "scenario_id=s1"],
        )

    records = queue.get_by_entity_uid(profile.config.uid).drain()
    assert len(records) == 1
    record = records[0]
    assert record.record_id == "turn-7"
    assert record.session_id == "session-123"
    assert record.media[0].id == "chart"
    assert "run_id=abc" in record.tags
    assert "scenario_id=s1" in record.tags
