from __future__ import annotations

import os
import time
from typing import Any

import pytest
import requests
from scouter.client import DriftRequest, TimeInterval
from scouter.types import DriftType

from .agents import build_runner, run_agent_turn
from .setup import AttachedEval, PromptSpec, ScouterUserJourney


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


def _page_agent_records(
    session: requests.Session,
    base_url: str,
    prompt: PromptSpec,
    status: str,
) -> list[dict[str, Any]]:
    response = session.post(
        f"{base_url}/scouter/agent/page/record",
        json={
            "service_info": {
                "space": prompt.eval_profile.config.space,
                "uid": prompt.eval_profile.config.uid,
            },
            "status": status,
            "limit": 100,
        },
        timeout=20,
    )
    response.raise_for_status()
    return response.json().get("items", [])


def _page_agent_workflows(
    session: requests.Session,
    base_url: str,
    prompt: PromptSpec,
) -> list[dict[str, Any]]:
    response = session.post(
        f"{base_url}/scouter/agent/page/workflow",
        json={
            "service_info": {
                "space": prompt.eval_profile.config.space,
                "uid": prompt.eval_profile.config.uid,
            },
            "limit": 100,
        },
        timeout=20,
    )
    response.raise_for_status()
    return response.json().get("items", [])


def _agent_tasks(
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


def _wait_for_processed_record(
    *,
    session: requests.Session,
    base_url: str,
    prompt: PromptSpec,
    attached_eval: AttachedEval,
    timeout_secs: float = 180.0,
) -> dict[str, Any]:
    deadline = time.monotonic() + timeout_secs
    last_processed: list[dict[str, Any]] = []

    while time.monotonic() < deadline:
        failed = _page_agent_records(session, base_url, prompt, status="Failed")
        failed_match = [record for record in failed if record.get("record_id") == attached_eval.record_id]
        if failed_match:
            raise AssertionError(f"Eval record failed: {failed_match}")

        last_processed = _page_agent_records(session, base_url, prompt, status="Processed")
        for record in last_processed:
            if record.get("record_id") == attached_eval.record_id:
                return record

        time.sleep(1)

    raise AssertionError(
        "Timed out waiting for processed eval record. "
        f"record_id={attached_eval.record_id} processed={last_processed}"
    )


def _wait_for_trace_spans(journey: ScouterUserJourney, trace_ids: set[str]) -> None:
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        if all(journey.client.get_trace_spans(trace_id).spans for trace_id in trace_ids):
            return
        time.sleep(1)
    raise AssertionError(f"Timed out waiting for trace spans: {sorted(trace_ids)}")


def _wait_for_workflow_result(
    *,
    session: requests.Session,
    base_url: str,
    prompt: PromptSpec,
    record_uid: str,
) -> dict[str, Any]:
    deadline = time.monotonic() + 60
    last_workflows: list[dict[str, Any]] = []

    while time.monotonic() < deadline:
        last_workflows = _page_agent_workflows(session, base_url, prompt)
        for workflow in last_workflows:
            if workflow.get("record_uid") == record_uid:
                return workflow
        time.sleep(1)

    raise AssertionError(f"Timed out waiting for workflow result: {last_workflows}")


def _wait_for_workflow_metric_stats(
    *,
    journey: ScouterUserJourney,
    prompt: PromptSpec,
    timeout_secs: float = 60.0,
) -> list[Any]:
    drift_request = DriftRequest(
        uid=prompt.eval_profile.config.uid,
        space=prompt.eval_profile.config.space,
        time_interval=TimeInterval.FifteenMinutes,
        max_data_points=10,
    )
    deadline = time.monotonic() + timeout_secs
    last_stats: list[Any] = []

    while time.monotonic() < deadline:
        workflow_metrics = journey.client.get_binned_drift(
            drift_request,
            drift_type=DriftType.Agent,
        )
        last_stats = list(getattr(workflow_metrics["workflow"], "stats", []))
        if last_stats:
            return last_stats
        time.sleep(1)

    raise AssertionError(f"Timed out waiting for workflow metric stats: {last_stats}")


def _assert_profile_outputs(
    *,
    journey: ScouterUserJourney,
    session: requests.Session,
    base_url: str,
    prompt: PromptSpec,
    attached_eval: AttachedEval,
) -> None:
    record = _wait_for_processed_record(
        session=session,
        base_url=base_url,
        prompt=prompt,
        attached_eval=attached_eval,
    )

    assert record["record_id"] == attached_eval.record_id
    assert record["session_id"] == attached_eval.session_id
    assert record["trace_id"] == attached_eval.trace_id
    assert record["span_id"] == attached_eval.span_id

    tasks = _agent_tasks(session, base_url, record["uid"])
    task_ids = {task["task_id"] for task in tasks}
    assert prompt.task_ids.issubset(task_ids)

    workflow = _wait_for_workflow_result(
        session=session,
        base_url=base_url,
        prompt=prompt,
        record_uid=record["uid"],
    )
    assert workflow["record_uid"] == record["uid"]
    assert workflow["total_tasks"] == len(prompt.task_ids)
    assert workflow["passed_tasks"] == workflow["total_tasks"]
    assert workflow["failed_tasks"] == 0
    assert workflow["pass_rate"] == 1.0

    workflow_stats = _wait_for_workflow_metric_stats(
        journey=journey,
        prompt=prompt,
    )
    assert workflow_stats


@pytest.mark.asyncio
async def test_scouter_multi_agent_user_journey(
    scouter_user_journey: ScouterUserJourney,
) -> None:
    runner, session_service = build_runner(scouter_user_journey)

    response = await run_agent_turn(
        runner,
        session_service,
        query="Help investigate a slow model response.",
    )

    assert "latency" in response.lower()
    assert {record.agent_name for record in scouter_user_journey.attached_evals} == {
        "router_agent",
        "resolution_agent",
    }

    trace_ids = {record.trace_id for record in scouter_user_journey.attached_evals}
    assert len(trace_ids) == 1
    assert len({record.span_id for record in scouter_user_journey.attached_evals}) == 2

    _wait_for_trace_spans(scouter_user_journey, trace_ids)
    scouter_user_journey.queue.shutdown()

    session = _auth_session(_server_url())
    by_agent = {record.agent_name: record for record in scouter_user_journey.attached_evals}

    _assert_profile_outputs(
        journey=scouter_user_journey,
        session=session,
        base_url=_server_url(),
        prompt=scouter_user_journey.router,
        attached_eval=by_agent["router_agent"],
    )
    _assert_profile_outputs(
        journey=scouter_user_journey,
        session=session,
        base_url=_server_url(),
        prompt=scouter_user_journey.resolution,
        attached_eval=by_agent["resolution_agent"],
    )
