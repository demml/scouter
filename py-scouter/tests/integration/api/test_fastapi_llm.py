from fastapi.testclient import TestClient
from scouter.client import DriftRequest, ScouterClient, TimeInterval
from scouter.types import DriftType

from tests.integration.api.conftest import PredictRequest

from .conftest import (
    create_and_register_drift_profile,
    create_http_app,
    create_kafka_app,
)


def test_api_kafka(kafka_scouter_server):
    # create the client
    scouter_client = ScouterClient()

    # create the drift profile
    profile = create_and_register_drift_profile(
        client=scouter_client, name="kafka_test"
    )
    drift_path = profile.save_to_json()
