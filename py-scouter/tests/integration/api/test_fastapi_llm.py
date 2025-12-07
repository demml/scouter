import time

import numpy as np
from fastapi.testclient import TestClient
from scouter.client import DriftRequest, ScouterClient, TimeInterval
from scouter.types import DriftType

from tests.integration.api.conftest import ChatRequest

from .conftest import create_and_register_llm_drift_profile, create_kafka_llm_app


def test_llm_api_kafka(kafka_scouter_openai_server):
    random_number = np.random.randint(0, 10)

    # create the client
    scouter_client = ScouterClient()

    # create the drift profile
    profile = create_and_register_llm_drift_profile(
        client=scouter_client,
        name=f"kafka_llm_test_{random_number}",
    )
    drift_path = profile.save_to_json()

    app = create_kafka_llm_app(drift_path)
    # Configure the TestClient
    with TestClient(app) as client:
        time.sleep(5)
        # Simulate requests
        for i in range(20):
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
        time_interval=TimeInterval.FiveMinutes,
        max_data_points=1,
    )

    metrics = scouter_client.get_binned_drift(
        request,
        drift_type=DriftType.LLM,
    )

    assert len(metrics["coherence"].stats) == 1
    assert metrics["coherence"].stats[0].avg > 0

    # delete the drift_path
    drift_path.unlink()
