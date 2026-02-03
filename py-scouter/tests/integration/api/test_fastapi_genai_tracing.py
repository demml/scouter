import time

import numpy as np
from fastapi.testclient import TestClient
from scouter.client import DriftRequest, ScouterClient, TimeInterval
from scouter.types import DriftType

from tests.integration.api.conftest import ChatRequest

from .conftest import create_and_register_genai_drift_profile, create_tracing_genai_app


def test_genai_tracing_api(scouter_grpc_openai_server):
    tracer, _server = scouter_grpc_openai_server
    random_number = np.random.randint(0, 10)

    # create the client
    scouter_client = ScouterClient()

    # create the drift profile
    profile = create_and_register_genai_drift_profile(
        client=scouter_client,
        name=f"grpc_genai_test_{random_number}",
    )
    drift_path = profile.save_to_json()

    app = create_tracing_genai_app(tracer, drift_path)
    # Configure the TestClient
    with TestClient(app) as client:
        time.sleep(5)
        # Simulate requests
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
        client.wait_shutdown()

    time.sleep(10)

    request = DriftRequest(
        uid=profile.uid,
        space=profile.config.space,
        time_interval=TimeInterval.FifteenMinutes,
        max_data_points=1,
    )

    workflow_results = scouter_client.get_binned_drift(
        request,
        drift_type=DriftType.GenAI,
    )

    assert len(workflow_results["workflow"].stats) == 1
    task_results = scouter_client.get_genai_task_binned_drift(request)
    assert len(task_results["coherence"].stats) == 1
    assert len(task_results["no_errors"].stats) == 1

    # get TestResponse record_uid from response
    record_uid = response.json().get("record_uid")
    assert record_uid is not None

    spans = scouter_client.get_trace_spans_from_tags(tags=[("scouter.queue.record", record_uid)])

    assert len(spans.spans) > 0

    # delete the drift_path
    drift_path.unlink()
