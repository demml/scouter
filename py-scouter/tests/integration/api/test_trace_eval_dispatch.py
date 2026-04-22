import json
import os
import time
import uuid
from collections import Counter
from typing import Any

import pytest
import requests
from opentelemetry import trace
from scouter import ScouterQueue
from scouter.client import ScouterClient
from scouter.drift import AgentEvalConfig, AgentEvalProfile, ComparisonOperator
from scouter.evaluate import (
    AssertionTask,
    EvalRecord,
    SpanFilter,
    TraceAssertion,
    TraceAssertionTask,
)
from scouter.mock import ScouterTestServer
from scouter.tracing import (
    BatchConfig,
    GrpcSpanExporter,
    ScouterInstrumentor,
    Tracer,
    active_profile,
    get_tracer,
    init_tracer,
    shutdown_tracer,
)
from scouter.transport import GrpcConfig


@pytest.fixture()
def _fast_trace_eval_env(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("GENAI_MAX_RETRIES", "5")
    monkeypatch.setenv("TRACE_EVAL_POLL_INTERVAL_SECS", "1")
    monkeypatch.setenv("TRACE_EVAL_LOOKBACK_SECS", "1200")
    monkeypatch.setenv("TRACE_EVAL_PROFILE_CACHE_TTL_SECS", "1")
    monkeypatch.setenv("GENAI_TRACE_WAIT_TIMEOUT_SECS", "2")
    monkeypatch.setenv("GENAI_TRACE_BACKOFF_MILLIS", "50")
    monkeypatch.setenv("GENAI_TRACE_RESCHEDULE_DELAY_SECS", "1")
    monkeypatch.setenv("SCOUTER_QUEUE_PUBLISH_INTERVAL_SECS", "1")
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


def _make_trace_profile(
    name: str,
    *,
    span_name: str = "mock_agent_workflow",
) -> AgentEvalProfile:
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
                assertion=TraceAssertion.span_exists(SpanFilter.by_name(span_name)),
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


def _make_queue_profile(name: str) -> AgentEvalProfile:
    return AgentEvalProfile(
        config=AgentEvalConfig(
            space="scouter",
            name=name,
            version="0.1.0",
            sample_ratio=1.0,
        ),
        tasks=[
            AssertionTask(
                id="assertion_ok",
                expected_value=10,
                context_path="assertion",
                operator=ComparisonOperator.Equals,
            ),
        ],
    )


def _query_agent_eval_records(
    session: requests.Session,
    base_url: str,
    profile: AgentEvalProfile,
    status: str | None = "Processed",
) -> list[dict[str, Any]]:
    body: dict[str, Any] = {
        "service_info": {"space": profile.config.space, "uid": profile.uid},
        "limit": 200,
    }
    if status is not None:
        body["status"] = status
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


def _query_trace_debug(
    scouter_client: ScouterClient,
    trace_id: str,
    profile_uids: list[str],
) -> dict[str, Any]:
    try:
        spans = scouter_client.get_trace_spans(trace_id).spans
    except Exception as exc:  # pragma: no cover - debug-only path
        return {"error": str(exc)}

    return {
        "span_count": len(spans),
        "span_names": [getattr(span, "span_name", "") for span in spans[:10]],
        "entity_tags_present": {
            profile_uid: any(
                str(_get_attr_value(span.attributes, f"scouter.entity.{profile_uid}")) == profile_uid for span in spans
            )
            for profile_uid in profile_uids
        },
    }


def _collect_trace_eval_debug_state(
    session: requests.Session,
    base_url: str,
    profile: AgentEvalProfile,
    trace_ids: list[str],
    scouter_client: ScouterClient,
    related_profiles: list[AgentEvalProfile] | None = None,
) -> dict[str, Any]:
    wanted = set(trace_ids)
    profiles = [profile, *(related_profiles or [])]
    profile_uids = [item.config.uid for item in profiles]
    status_debug: dict[str, list[dict[str, Any]]] = {}

    for status in ("Pending", "Processing", "Processed", "Failed"):
        records = _query_agent_eval_records(session, base_url, profile, status=status)
        matched_records = []

        for record in records:
            trace_id = record.get("trace_id")
            if trace_id not in wanted:
                continue

            record_uid = record.get("uid") or record.get("record_uid")
            tasks = _query_agent_tasks(session, base_url, record_uid) if record_uid else []
            matched_records.append(
                {
                    "trace_id": trace_id,
                    "record_uid": record_uid,
                    "record_source": record.get("record_source"),
                    "retry_count": record.get("retry_count"),
                    "processing_started_at": record.get("processing_started_at"),
                    "processing_ended_at": record.get("processing_ended_at"),
                    "processing_duration": record.get("processing_duration"),
                    "tasks": _task_pass_map(tasks),
                }
            )

        status_debug[status] = matched_records

    return {
        "profile_name": profile.config.name,
        "profile_uid": profile.config.uid,
        "wanted_trace_ids": sorted(wanted),
        "records_by_status": status_debug,
        "trace_debug": {
            trace_id: _query_trace_debug(scouter_client, trace_id, profile_uids) for trace_id in sorted(wanted)
        },
    }


def _wait_for_trace_spans_visible(
    scouter_client: ScouterClient,
    trace_ids: list[str],
    profile_uids: list[str],
    timeout_secs: float = 30.0,
) -> dict[str, dict[str, Any]]:
    wanted = set(trace_ids)
    deadline = time.time() + timeout_secs

    while time.time() < deadline:
        trace_debug = {
            trace_id: _query_trace_debug(scouter_client, trace_id, profile_uids) for trace_id in sorted(wanted)
        }
        if all(trace_debug[trace_id].get("span_count", 0) > 0 for trace_id in wanted):
            return trace_debug
        time.sleep(1.0)

    raise AssertionError(
        "Timed out waiting for trace spans to become visible "
        f"for traces={sorted(wanted)} "
        f"trace_debug={json.dumps(trace_debug, indent=2, sort_keys=True)}"
    )


def _wait_for_trace_eval_records_written(
    session: requests.Session,
    base_url: str,
    profile: AgentEvalProfile,
    trace_ids: list[str],
    scouter_client: ScouterClient,
    timeout_secs: float = 30.0,
    related_profiles: list[AgentEvalProfile] | None = None,
) -> dict[str, str]:
    wanted = set(trace_ids)
    deadline = time.time() + timeout_secs

    while time.time() < deadline:
        status_by_trace: dict[str, str] = {}

        for status in ("Pending", "Processing", "Processed", "Failed"):
            records = _query_agent_eval_records(session, base_url, profile, status=status)
            for record in records:
                trace_id = record.get("trace_id")
                if trace_id in wanted and trace_id not in status_by_trace:
                    status_by_trace[trace_id] = status

        if len(status_by_trace) == len(wanted):
            return status_by_trace

        time.sleep(1.0)

    debug_state = _collect_trace_eval_debug_state(
        session,
        base_url,
        profile,
        trace_ids,
        scouter_client,
        related_profiles=related_profiles,
    )
    raise AssertionError(
        "Timed out waiting for trace eval records to be created "
        f"for traces={sorted(wanted)} profile={profile.config.name}\n"
        f"debug_state={json.dumps(debug_state, indent=2, sort_keys=True)}"
    )


def _wait_for_processed_trace_evals(
    session: requests.Session,
    base_url: str,
    profile: AgentEvalProfile,
    trace_ids: list[str],
    scouter_client: ScouterClient,
    timeout_secs: float = 90.0,
    related_profiles: list[AgentEvalProfile] | None = None,
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

    debug_state = _collect_trace_eval_debug_state(
        session,
        base_url,
        profile,
        trace_ids,
        scouter_client,
        related_profiles=related_profiles,
    )
    raise AssertionError(
        "Timed out waiting for processed trace eval records "
        f"for traces={sorted(wanted)} profile={profile.config.name}\n"
        f"debug_state={json.dumps(debug_state, indent=2, sort_keys=True)}"
    )


def _normalize_record_source(value: Any) -> str:
    return str(value or "").replace("_", "").lower()


def _wait_for_processed_agent_evals(
    session: requests.Session,
    base_url: str,
    profile: AgentEvalProfile,
    trace_ids: list[str],
    scouter_client: ScouterClient,
    expected_tasks: set[str],
    expected_source: str,
    timeout_secs: float = 90.0,
    related_profiles: list[AgentEvalProfile] | None = None,
) -> dict[str, dict[str, Any]]:
    wanted = set(trace_ids)
    deadline = time.time() + timeout_secs
    normalized_source = _normalize_record_source(expected_source)

    while time.time() < deadline:
        records = _query_agent_eval_records(session, base_url, profile)
        matched: dict[str, dict[str, Any]] = {}
        counts: Counter[str] = Counter()

        for record in records:
            trace_id = record.get("trace_id")
            if trace_id not in wanted:
                continue
            if _normalize_record_source(record.get("record_source")) != normalized_source:
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

    debug_state = _collect_trace_eval_debug_state(
        session,
        base_url,
        profile,
        trace_ids,
        scouter_client,
        related_profiles=related_profiles,
    )
    raise AssertionError(
        "Timed out waiting for processed agent eval records "
        f"for traces={sorted(wanted)} profile={profile.config.name} expected_source={expected_source}\n"
        f"debug_state={json.dumps(debug_state, indent=2, sort_keys=True)}"
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


def _run_mixed_agent_workflow(
    tracer: Tracer,
    planner_profile: AgentEvalProfile,
) -> str:
    with tracer.start_as_current_span("agent_1_orchestrator") as orchestrator_span:
        orchestrator_span.set_attribute("agent.name", "orchestrator")
        orchestrator_span.set_attribute("workflow.kind", "integration_test")
        orchestrator_span.add_queue_item(
            alias="orchestrator",
            item=EvalRecord(
                context={
                    "agent": "orchestrator",
                    "assertion": 10,
                },
            ),
        )

        with active_profile(planner_profile):
            with tracer.start_as_current_span("agent_2_planner") as planner_span:
                planner_span.set_attribute("agent.name", "planner")
                planner_span.set_attribute("workflow.kind", "integration_test")

        with tracer.start_as_current_span("agent_3_analyzer") as analyzer_span:
            analyzer_span.set_attribute("agent.name", "analyzer")
            analyzer_span.set_attribute("workflow.kind", "integration_test")
            analyzer_span.add_queue_item(
                alias="analyzer",
                item=EvalRecord(
                    context={
                        "agent": "analyzer",
                        "assertion": 10,
                    },
                ),
            )

        return str(orchestrator_span.trace_id)


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

            _wait_for_trace_spans_visible(scouter_client, [trace_id], [profile.config.uid], timeout_secs=30.0)
            _wait_for_trace_eval_records_written(
                session,
                base_url,
                profile,
                [trace_id],
                scouter_client,
                timeout_secs=30.0,
            )
            _wait_for_processed_trace_evals(
                session,
                base_url,
                profile,
                [trace_id],
                scouter_client,
                timeout_secs=90.0,
            )

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

            _wait_for_trace_spans_visible(
                scouter_client,
                [trace_id_a, trace_id_b],
                [profile_a.config.uid, profile_b.config.uid],
                timeout_secs=30.0,
            )
            _wait_for_trace_eval_records_written(
                session,
                base_url,
                profile_a,
                [trace_id_a],
                scouter_client,
                timeout_secs=30.0,
                related_profiles=[profile_b],
            )
            _wait_for_trace_eval_records_written(
                session,
                base_url,
                profile_b,
                [trace_id_b],
                scouter_client,
                timeout_secs=30.0,
                related_profiles=[profile_a],
            )
            _wait_for_processed_trace_evals(
                session,
                base_url,
                profile_a,
                [trace_id_a],
                scouter_client,
                timeout_secs=90.0,
                related_profiles=[profile_b],
            )
            _wait_for_processed_trace_evals(
                session,
                base_url,
                profile_b,
                [trace_id_b],
                scouter_client,
                timeout_secs=90.0,
                related_profiles=[profile_a],
            )

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


def test_trace_eval_dispatch_mixed_queue_and_synthetic_entities(
    _fast_trace_eval_env: None,
    isolated_server_config,
):
    orchestrator_profile = _make_queue_profile(f"trace_eval_orchestrator_{uuid.uuid4().hex[:8]}")
    planner_profile = _make_trace_profile(
        f"trace_eval_planner_{uuid.uuid4().hex[:8]}",
        span_name="agent_2_planner",
    )
    analyzer_profile = _make_queue_profile(f"trace_eval_analyzer_{uuid.uuid4().hex[:8]}")

    with ScouterTestServer(**isolated_server_config) as _server:
        base_url = _server_url()
        session = _auth_session(base_url)
        scouter_client = ScouterClient()

        assert scouter_client.register_profile(orchestrator_profile, set_active=True, deactivate_others=False)
        assert scouter_client.register_profile(planner_profile, set_active=True, deactivate_others=False)
        assert scouter_client.register_profile(analyzer_profile, set_active=True, deactivate_others=False)

        base_path = isolated_server_config["base_path"]
        orchestrator_path = orchestrator_profile.save_to_json(
            base_path / f"{orchestrator_profile.config.uid}_orchestrator"
        )
        analyzer_path = analyzer_profile.save_to_json(base_path / f"{analyzer_profile.config.uid}_analyzer")
        queue = ScouterQueue.from_path(
            path={
                "orchestrator": orchestrator_path,
                "analyzer": analyzer_path,
            },
            transport_config=GrpcConfig(),
        )

        try:
            init_tracer(
                service_name="mixed-trace-eval-test",
                exporter=GrpcSpanExporter(),
                batch_config=BatchConfig(scheduled_delay_ms=200),
            )
            tracer = get_tracer("mixed-trace-eval-test")
            tracer.set_scouter_queue(queue)

            trace_id = _run_mixed_agent_workflow(
                tracer=tracer,
                planner_profile=planner_profile,
            )
            time.sleep(2.0)
            queue.shutdown()
            shutdown_tracer()

            _wait_for_trace_spans_visible(
                scouter_client,
                [trace_id],
                [
                    orchestrator_profile.config.uid,
                    planner_profile.config.uid,
                    analyzer_profile.config.uid,
                ],
                timeout_secs=30.0,
            )

            _wait_for_trace_eval_records_written(
                session,
                base_url,
                planner_profile,
                [trace_id],
                scouter_client,
                timeout_secs=30.0,
                related_profiles=[orchestrator_profile, analyzer_profile],
            )

            _wait_for_processed_agent_evals(
                session,
                base_url,
                orchestrator_profile,
                [trace_id],
                scouter_client,
                expected_tasks={"assertion_ok"},
                expected_source="queue",
                timeout_secs=90.0,
                related_profiles=[planner_profile, analyzer_profile],
            )
            _wait_for_processed_trace_evals(
                session,
                base_url,
                planner_profile,
                [trace_id],
                scouter_client,
                timeout_secs=90.0,
                related_profiles=[orchestrator_profile, analyzer_profile],
            )
            _wait_for_processed_agent_evals(
                session,
                base_url,
                analyzer_profile,
                [trace_id],
                scouter_client,
                expected_tasks={"assertion_ok"},
                expected_source="queue",
                timeout_secs=90.0,
                related_profiles=[orchestrator_profile, planner_profile],
            )

            spans = scouter_client.get_trace_spans(trace_id).spans
            assert len(spans) > 0

            planner_key = f"scouter.entity.{planner_profile.config.uid}"

            assert any(
                str(_get_attr_value(span.attributes, planner_key)) == planner_profile.config.uid for span in spans
            ), "Expected planner span attributes to include the trace-only entity UID tag"

            orchestrator_records = _query_agent_eval_records(session, base_url, orchestrator_profile)
            planner_records = _query_agent_eval_records(session, base_url, planner_profile)
            analyzer_records = _query_agent_eval_records(session, base_url, analyzer_profile)

            assert sum(1 for record in orchestrator_records if record.get("trace_id") == trace_id) == 1
            assert sum(1 for record in planner_records if record.get("trace_id") == trace_id) == 1
            assert sum(1 for record in analyzer_records if record.get("trace_id") == trace_id) == 1
            assert any(
                _normalize_record_source(record.get("record_source")) == "queue"
                for record in orchestrator_records
                if record.get("trace_id") == trace_id
            )
            assert any(
                _normalize_record_source(record.get("record_source")) == "tracedispatch"
                for record in planner_records
                if record.get("trace_id") == trace_id
            )
            assert any(
                _normalize_record_source(record.get("record_source")) == "queue"
                for record in analyzer_records
                if record.get("trace_id") == trace_id
            )
        finally:
            queue.shutdown()
            shutdown_tracer()
            orchestrator_path.unlink(missing_ok=True)
            analyzer_path.unlink(missing_ok=True)
