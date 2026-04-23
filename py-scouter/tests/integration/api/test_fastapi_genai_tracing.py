import time

import numpy as np
import pytest
from fastapi.testclient import TestClient
from scouter.client import ScouterClient, TraceFilters

from tests.integration.api.conftest import ChatRequest

from .conftest import create_and_register_agent_drift_profile, create_tracing_agent_app


@pytest.fixture()
def _fast_agent_tracing_env(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("SCOUTER_QUEUE_PUBLISH_INTERVAL_SECS", "1")


def test_agent_tracing_api(_fast_agent_tracing_env: None, scouter_grpc_openai_server):
    tracer, _server = scouter_grpc_openai_server
    random_number = np.random.randint(0, 10)

    scouter_client = ScouterClient()

    profile = create_and_register_agent_drift_profile(
        client=scouter_client,
        name=f"grpc_genai_test_{random_number}",
        with_trace_assertion=True,
    )
    drift_path = profile.save_to_json()

    app = create_tracing_agent_app(tracer, drift_path)
    with TestClient(app) as client:
        time.sleep(5)
        for i in range(30):
            response = client.post(
                "/chat",
                json=ChatRequest(
                    question=f"Have you ever heard of the band Turnstile?, Request number: {i}",
                ).model_dump(),
            )
            assert response.status_code == 200
            time.sleep(0.5)
        time.sleep(5)

    record_uid = response.json().get("record_uid")
    assert record_uid is not None

    deadline = time.time() + 30
    spans = None
    while time.time() < deadline:
        spans = scouter_client.get_trace_spans_from_filters(filters=TraceFilters(queue_uid=record_uid))
        if spans.spans:
            break
        time.sleep(1)

    assert spans is not None
    assert len(spans.spans) > 0

    drift_path.unlink()
