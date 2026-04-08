import time

import numpy as np
from fastapi.testclient import TestClient
from scouter.client import DriftRequest, ScouterClient, TimeInterval
from scouter.types import DriftType

from tests.integration.api.conftest import ChatRequest

from .conftest import create_and_register_agent_drift_profile, create_kafka_agent_app


def test_agent_api_kafka(kafka_scouter_openai_server):
    random_number = np.random.randint(0, 10)

    # create the client
    scouter_client = ScouterClient()

    # create the drift profile
    profile = create_and_register_agent_drift_profile(
        client=scouter_client,
        name=f"kafka_genai_test_{random_number}",
        with_trace_assertion=False,
    )
    drift_path = profile.save_to_json()

    app = create_kafka_agent_app(drift_path)

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

        # flush the queue
        response = client.post("/flush")
        assert response.status_code == 200

        client.wait_shutdown()  # type: ignore

    time.sleep(10)

    request = DriftRequest(
        uid=profile.uid,
        space=profile.config.space,
        time_interval=TimeInterval.FifteenMinutes,
        max_data_points=1,
    )

    # workflow metrics
    workflow_results = scouter_client.get_binned_drift(
        request,
        drift_type=DriftType.Agent,
    )

    assert len(workflow_results["workflow"].stats) == 1
    task_results = scouter_client.get_agent_task_binned_drift(request)
    assert len(task_results["coherence"].stats) == 1

    drift_path.unlink()
