import os
import time
import uuid
from typing import Any

import pytest
import requests
from scouter import ScouterQueue
from scouter.agent import Provider
from scouter.client import DriftRequest, ScouterClient, TimeInterval
from scouter.drift import AgentEvalConfig, AgentEvalProfile, ComparisonOperator, Drifter
from scouter.evaluate import (
    AgentAssertion,
    AgentAssertionTask,
    AssertionTask,
    EvalRecord,
    LLMJudgeTask,
    TraceAssertion,
    TraceAssertionTask,
)
from scouter.mock import ScouterTestServer
from scouter.tracing import (
    BatchConfig,
    GrpcSpanExporter,
    ScouterInstrumentor,
    active_profile,
    flush_tracer,
    init_tracer,
    shutdown_tracer,
)
from scouter.transport import GrpcConfig
from scouter.types import DriftType

from tests.integration.api.conftest import create_coherence_evaluation_prompt


@pytest.fixture()
def _fast_pipeline_env(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("SCOUTER_QUEUE_PUBLISH_INTERVAL_SECS", "1")
    monkeypatch.setenv("GENAI_MAX_RETRIES", "8")
    monkeypatch.setenv("GENAI_TRACE_WAIT_TIMEOUT_SECS", "15")
    monkeypatch.setenv("GENAI_TRACE_RESCHEDULE_DELAY_SECS", "1")
    monkeypatch.setenv("TRACE_EVAL_POLL_INTERVAL_SECS", "1")
    monkeypatch.setenv("TRACE_EVAL_PROFILE_CACHE_TTL_SECS", "1")


def _server_url() -> str:
    return os.environ.get("SCOUTER_SERVER_URI", "http://localhost:3000")


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


def _query_agent_eval_records(
    session: requests.Session,
    base_url: str,
    profile: AgentEvalProfile,
    status: str,
) -> list[dict[str, Any]]:
    body: dict[str, Any] = {
        "service_info": {"space": profile.config.space, "uid": profile.uid},
        "status": status,
        "limit": 500,
    }
    response = session.post(
        f"{base_url}/scouter/agent/page/record",
        json=body,
        timeout=20,
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
        timeout=20,
    )
    response.raise_for_status()
    return response.json().get("tasks", [])


def _wait_for_processed_records(
    session: requests.Session,
    base_url: str,
    profile: AgentEvalProfile,
    expected_uids: set[str],
    required_task_ids: set[str],
    timeout_secs: float = 180.0,
) -> list[dict[str, Any]]:
    deadline = time.time() + timeout_secs
    processed_matches: list[dict[str, Any]] = []

    while time.time() < deadline:
        processed = _query_agent_eval_records(session, base_url, profile, status="Processed")
        processed_matches = [
            record for record in processed if (record.get("uid") or record.get("record_uid")) in expected_uids
        ]

        if len(processed_matches) == len(expected_uids):
            all_tasks_present = True
            for record in processed_matches:
                record_uid = record.get("uid") or record.get("record_uid")
                if not record_uid:
                    all_tasks_present = False
                    break
                tasks = _query_agent_tasks(session, base_url, str(record_uid))
                task_ids = {str(task.get("task_id", "")) for task in tasks}
                if not required_task_ids.issubset(task_ids):
                    all_tasks_present = False
                    break

            if all_tasks_present:
                return processed_matches

        failed = _query_agent_eval_records(session, base_url, profile, status="Failed")
        failed_matches = [
            record for record in failed if (record.get("uid") or record.get("record_uid")) in expected_uids
        ]
        if failed_matches:
            raise AssertionError(f"Found failed eval records: {failed_matches}")

        time.sleep(1)

    raise AssertionError(
        "Timed out waiting for processed eval records. "
        f"expected_uids={sorted(expected_uids)} processed={processed_matches}"
    )


def _wait_for_drift_results(
    client: ScouterClient,
    profile: AgentEvalProfile,
    task_ids: set[str],
    timeout_secs: float = 120.0,
) -> tuple[Any, Any]:
    deadline = time.time() + timeout_secs

    request = DriftRequest(
        uid=profile.uid,
        space=profile.config.space,
        time_interval=TimeInterval.FifteenMinutes,
        max_data_points=10,
    )

    workflow_results: Any = None
    task_results: Any = None

    while time.time() < deadline:
        workflow_results = client.get_binned_drift(request, drift_type=DriftType.Agent)
        task_results = client.get_agent_task_binned_drift(request)

        workflow_stats = getattr(workflow_results["workflow"], "stats", [])
        all_task_stats_ready = all(getattr(task_results[task_id], "stats", []) for task_id in task_ids)

        if workflow_stats and all_task_stats_ready:
            return workflow_results, task_results

        time.sleep(2)

    task_keys = list(task_results.keys()) if task_results is not None else []
    raise AssertionError("Timed out waiting for drift results. " f"workflow={workflow_results} task_keys={task_keys}")


def _wait_for_trace_spans_visible(
    client: ScouterClient,
    trace_ids: set[str],
    timeout_secs: float = 30.0,
) -> None:
    deadline = time.time() + timeout_secs
    while time.time() < deadline:
        if all(client.get_trace_spans(trace_id).spans for trace_id in trace_ids):
            return
        time.sleep(1)
    raise AssertionError(f"Timed out waiting for trace spans: {sorted(trace_ids)}")


def test_agent_eval_pipeline_e2e_with_instrumentor(
    _fast_pipeline_env: None,
    isolated_server_config,
):
    config = AgentEvalConfig(
        space="scouter",
        name=f"agent_pipeline_e2e_{uuid.uuid4().hex[:8]}",
        version="0.1.0",
        sample_ratio=1.0,
    )

    tasks: list[LLMJudgeTask | AssertionTask | TraceAssertionTask | AgentAssertionTask] = [
        AssertionTask(
            id="assertion_ok",
            expected_value=10,
            context_path="assertion",
            operator=ComparisonOperator.Equals,
        ),
        LLMJudgeTask(
            id="coherence",
            expected_value=4,
            prompt=create_coherence_evaluation_prompt(),
            context_path="score",
            operator=ComparisonOperator.GreaterThanOrEqual,
        ),
        TraceAssertionTask(
            id="no_errors",
            assertion=TraceAssertion.trace_error_count(),
            expected_value=0,
            operator=ComparisonOperator.Equals,
        ),
        AgentAssertionTask(
            id="agent_response_content",
            assertion=AgentAssertion.response_content(),
            context_path="agent_response",
            expected_value="Turnstile",
            operator=ComparisonOperator.Contains,
            provider=Provider.OpenAI,
        ),
    ]

    with ScouterTestServer(openai=True, **isolated_server_config) as _server:
        client = ScouterClient()
        profile = Drifter().create_agent_drift_profile(config=config, tasks=tasks)
        assert client.register_profile(profile, set_active=True)

        profile_path = profile.save_to_json(isolated_server_config["base_path"] / profile.config.uid)

        queue = ScouterQueue.from_path(path={"agent": profile_path}, transport_config=GrpcConfig())
        instrumentor = ScouterInstrumentor()
        instrumentor.instrument(
            transport_config=GrpcConfig(),
            exporter=GrpcSpanExporter(),
            batch_config=BatchConfig(scheduled_delay_ms=200),
            scouter_queue=queue,
            eval_profiles=[profile],
        )

        tracer = init_tracer(
            service_name="agent-e2e-pipeline",
            transport_config=GrpcConfig(),
            exporter=GrpcSpanExporter(),
            batch_config=BatchConfig(scheduled_delay_ms=200),
            scouter_queue=queue,
            default_entity_uid=profile.config.uid,
        )
        record_uids: set[str] = set()
        trace_ids: set[str] = set()

        try:
            with active_profile(profile):
                for i in range(5):
                    with tracer.start_as_current_span("agent_loop_step") as span:
                        response_content = "Turnstile are a hardcore band from Baltimore. " f"Iteration {i}."
                        agent_response = {
                            "model": "gpt-4o",
                            "choices": [
                                {
                                    "message": {
                                        "role": "assistant",
                                        "content": response_content,
                                    },
                                    "finish_reason": "stop",
                                }
                            ],
                            "usage": {
                                "prompt_tokens": 10,
                                "completion_tokens": 40,
                                "total_tokens": 50,
                            },
                        }

                        record = EvalRecord(
                            context={
                                "input": f"Tell me about Turnstile {i}",
                                "response": response_content,
                                "assertion": 10,
                                "agent_response": agent_response,
                            }
                        )
                        span.add_queue_item(alias="agent", item=record)
                        record_uids.add(record.uid)
                        trace_ids.add(str(span.trace_id))

            flush_tracer()
            _wait_for_trace_spans_visible(client=client, trace_ids=trace_ids)
            queue.shutdown()

            required_task_ids = {
                "assertion_ok",
                "coherence",
                "no_errors",
                "agent_response_content",
            }

            session = _auth_session(_server_url())
            processed_records = _wait_for_processed_records(
                session=session,
                base_url=_server_url(),
                profile=profile,
                expected_uids=record_uids,
                required_task_ids=required_task_ids,
            )
            assert len(processed_records) == len(record_uids)

            workflow_results, task_results = _wait_for_drift_results(
                client=client,
                profile=profile,
                task_ids=required_task_ids,
            )

            assert len(getattr(workflow_results["workflow"], "stats", [])) > 0
            for task_id in required_task_ids:
                assert len(getattr(task_results[task_id], "stats", [])) > 0

        finally:
            shutdown_tracer()
            instrumentor.uninstrument()
            queue.shutdown()
            profile_path.unlink(missing_ok=True)
