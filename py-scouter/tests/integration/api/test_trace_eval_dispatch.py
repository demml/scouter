import os
import time
import uuid
from collections import Counter
from typing import Any

import pytest
import requests
from opentelemetry import trace
from scouter.client import ScouterClient
from scouter.drift import AgentEvalConfig, AgentEvalProfile, ComparisonOperator
from scouter.evaluate import SpanFilter, TraceAssertion, TraceAssertionTask
from scouter.mock import ScouterTestServer
from scouter.tracing import (
    BatchConfig,
    GrpcSpanExporter,
    ScouterInstrumentor,
    active_profile,
)
from scouter.transport import GrpcConfig


@pytest.fixture()
def _fast_trace_eval_env(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("TRACE_EVAL_POLL_INTERVAL_SECS", "1")
    monkeypatch.setenv("TRACE_EVAL_LOOKBACK_SECS", "1200")
    monkeypatch.setenv("TRACE_EVAL_PROFILE_CACHE_TTL_SECS", "1")
    monkeypatch.setenv("GENAI_TRACE_WAIT_TIMEOUT_SECS", "2")
    monkeypatch.setenv("GENAI_TRACE_BACKOFF_MILLIS", "50")
    monkeypatch.setenv("GENAI_TRACE_RESCHEDULE_DELAY_SECS", "1")
    monkeypatch.setenv("SCOUTER_TRACE_REFRESH_INTERVAL_SECS", "1")


def _auth_session(base_url: str) -> requests.Session:
    response = requests.get(
        f"{base_url}/scouter/auth/login",
        headers={"Username": "admin", "Password": "admin"},
        timeout=15,
    )
    response.raise_for_status()
    token = response.json()["token"]

    session = requests.Session()
    session.headers.update({"Authorization": f"Bearer {token}"})
    return session


def _server_url() -> str:
    return os.environ.get("SCOUTER_SERVER_URI", "http://localhost:3000")


def _make_trace_profile(name: str) -> AgentEvalProfile:
    return AgentEvalProfile(
        config=AgentEvalConfig(
            space="scouter",
            name=name,
            version="0.1.0",
            sample_ratio=1.0,
        ),
        tasks=[
            TraceAssertionTask(
                id="has_workflow_span",
                assertion=TraceAssertion.span_exists(SpanFilter.by_name("mock_agent_workflow")),
                expected_value=True,
                operator=ComparisonOperator.Equals,
            ),
            TraceAssertionTask(
                id="no_errors",
                assertion=TraceAssertion.trace_error_count(),
                expected_value=0,
                operator=ComparisonOperator.Equals,
            ),
        ],
    )


def _query_agent_eval_records(
    session: requests.Session,
    base_url: str,
    profile: AgentEvalProfile,
) -> list[dict[str, Any]]:
    body = {
        "service_info": {"space": profile.config.space, "uid": profile.uid},
        "limit": 200,
        "status": "Processed",
    }
    response = session.post(
        f"{base_url}/scouter/agent/page/record",
        json=body,
        timeout=15,
    )
    response.raise_for_status()
    return response.json().get("items", [])


def _query_agent_tasks(
    session: requests.Session,
    base_url: str,
    record_uid: str,
) -> list[dict[str, Any]]:
    response = session.get(
        f"{base_url}/scouter/agent/task",
        params={"record_uid": record_uid},
        timeout=15,
    )
    response.raise_for_status()
    return response.json().get("tasks", [])


def _task_pass_map(tasks: list[dict[str, Any]]) -> dict[str, bool]:
    return {task.get("task_id", ""): bool(task.get("passed", False)) for task in tasks}


def _wait_for_processed_trace_evals(
    session: requests.Session,
    base_url: str,
    profile: AgentEvalProfile,
    trace_ids: list[str],
    timeout_secs: float = 90.0,
) -> dict[str, dict[str, Any]]:
    expected_tasks = {"has_workflow_span", "no_errors"}
    wanted = set(trace_ids)
    deadline = time.time() + timeout_secs

    while time.time() < deadline:
        records = _query_agent_eval_records(session, base_url, profile)
        matched: dict[str, dict[str, Any]] = {}
        counts: Counter[str] = Counter()

        for record in records:
            trace_id = record.get("trace_id")
            if trace_id not in wanted:
                continue

            counts[trace_id] += 1
            record_uid = record.get("uid") or record.get("record_uid")
            if not record_uid:
                continue

            tasks = _query_agent_tasks(session, base_url, record_uid)
            pass_map = _task_pass_map(tasks)
            if expected_tasks.issubset(pass_map.keys()) and all(pass_map[task] for task in expected_tasks):
                matched[trace_id] = record

        if len(matched) == len(wanted):
            for trace_id in wanted:
                assert counts[trace_id] == 1, f"Duplicate eval records found for trace_id={trace_id}"
            return matched

        time.sleep(1.0)

    raise AssertionError(
        f"Timed out waiting for processed trace eval records for traces={sorted(wanted)} profile={profile.config.name}"
    )


def _get_attr_value(span_attributes: list[Any], key: str) -> Any:
    for attr in span_attributes:
        if attr.key == key:
            return attr.value
    return None


def _run_mock_agent_workflow(tracer: Any, agent_name: str) -> str:
    with tracer.start_as_current_span("mock_agent_workflow") as span:
        span.set_attribute("agent.name", agent_name)
        span.set_attribute("workflow.kind", "integration_test")
        return str(span.trace_id)


def test_trace_eval_dispatch_from_auto_instrumented_trace(
    _fast_trace_eval_env: None,
    isolated_server_config,
):
    profile = _make_trace_profile(f"trace_eval_auto_{uuid.uuid4().hex[:8]}")

    with ScouterTestServer(**isolated_server_config) as _server:
        base_url = _server_url()
        session = _auth_session(base_url)

        scouter_client = ScouterClient()
        assert scouter_client.register_profile(profile, set_active=True, deactivate_others=False)

        instrumentor = ScouterInstrumentor()
        instrumentor.instrument(
            transport_config=GrpcConfig(),
            exporter=GrpcSpanExporter(),
            batch_config=BatchConfig(scheduled_delay_ms=200),
            eval_profiles=[profile],
        )

        try:
            tracer = trace.get_tracer("trace-eval-auto")
            trace_id = _run_mock_agent_workflow(tracer=tracer, agent_name="single_agent")

            _wait_for_processed_trace_evals(session, base_url, profile, [trace_id], timeout_secs=90.0)

            spans = scouter_client.get_trace_spans(trace_id).spans
            assert len(spans) > 0
            entity_key = f"scouter.entity.{profile.config.uid}"
            assert any(
                str(_get_attr_value(span.attributes, entity_key)) == profile.config.uid for span in spans
            ), "Expected span attributes to include the profile entity UID tag"

            time.sleep(3)
            records = _query_agent_eval_records(session, base_url, profile)
            trace_record_count = sum(1 for record in records if record.get("trace_id") == trace_id)
            assert trace_record_count == 1, "Synthetic eval record should not be duplicated"
        finally:
            instrumentor.uninstrument()


def test_trace_eval_dispatch_multi_agent_active_profile_switching(
    _fast_trace_eval_env: None,
    isolated_server_config,
):
    profile_a = _make_trace_profile(f"trace_eval_alpha_{uuid.uuid4().hex[:8]}")
    profile_b = _make_trace_profile(f"trace_eval_beta_{uuid.uuid4().hex[:8]}")

    with ScouterTestServer(**isolated_server_config) as _server:
        base_url = _server_url()
        session = _auth_session(base_url)

        scouter_client = ScouterClient()
        assert scouter_client.register_profile(profile_a, set_active=True, deactivate_others=False)
        assert scouter_client.register_profile(profile_b, set_active=True, deactivate_others=False)

        instrumentor = ScouterInstrumentor()
        instrumentor.instrument(
            transport_config=GrpcConfig(),
            exporter=GrpcSpanExporter(),
            batch_config=BatchConfig(scheduled_delay_ms=200),
            eval_profiles=[profile_a, profile_b],
            propagate_baggage=True,
        )

        try:
            tracer = trace.get_tracer("trace-eval-multi")

            trace_id_a = _run_mock_agent_workflow(tracer=tracer, agent_name="alpha_agent")

            with active_profile(profile_b):
                trace_id_b = _run_mock_agent_workflow(tracer=tracer, agent_name="beta_agent")

            _wait_for_processed_trace_evals(session, base_url, profile_a, [trace_id_a], timeout_secs=90.0)
            _wait_for_processed_trace_evals(session, base_url, profile_b, [trace_id_b], timeout_secs=90.0)

            records_a = _query_agent_eval_records(session, base_url, profile_a)
            records_b = _query_agent_eval_records(session, base_url, profile_b)

            assert sum(1 for record in records_a if record.get("trace_id") == trace_id_a) == 1
            assert sum(1 for record in records_b if record.get("trace_id") == trace_id_b) == 1
            assert sum(1 for record in records_a if record.get("trace_id") == trace_id_b) == 0
            assert sum(1 for record in records_b if record.get("trace_id") == trace_id_a) == 0

            spans_a = scouter_client.get_trace_spans(trace_id_a).spans
            spans_b = scouter_client.get_trace_spans(trace_id_b).spans
            assert len(spans_a) > 0 and len(spans_b) > 0

            key_a = f"scouter.entity.{profile_a.config.uid}"
            key_b = f"scouter.entity.{profile_b.config.uid}"

            assert any(str(_get_attr_value(span.attributes, key_a)) == profile_a.config.uid for span in spans_a)
            assert any(str(_get_attr_value(span.attributes, key_b)) == profile_b.config.uid for span in spans_b)
            assert not any(str(_get_attr_value(span.attributes, key_b)) == profile_b.config.uid for span in spans_a)
            assert not any(str(_get_attr_value(span.attributes, key_a)) == profile_a.config.uid for span in spans_b)

            time.sleep(3)
            records_a_after = _query_agent_eval_records(session, base_url, profile_a)
            records_b_after = _query_agent_eval_records(session, base_url, profile_b)
            assert sum(1 for record in records_a_after if record.get("trace_id") == trace_id_a) == 1
            assert sum(1 for record in records_b_after if record.get("trace_id") == trace_id_b) == 1
        finally:
            instrumentor.uninstrument()
