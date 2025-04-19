import time

from fastapi.testclient import TestClient
from scouter.client import DriftRequest, ScouterClient, TimeInterval
from scouter.logging import LoggingConfig, LogLevel, RustyLogger
from scouter.types import DriftType

from tests.integration.api.conftest import PredictRequest

from .conftest import create_and_register_drift_profile, create_app

RustyLogger.setup_logging(
    LoggingConfig(log_level=LogLevel.Debug),
)


def test_router_mixin_kafka(kafka_scouter_server):
    scouter_client = ScouterClient()

    drift_profile = create_and_register_drift_profile(client=scouter_client)

    # Get the test client
    client = TestClient(create_app(drift_profile))

    for i in range(60):
        response = client.post(
            "/predict",
            json=PredictRequest(
                feature_0=1.0,
                feature_1=1.0,
                feature_2=1.0,
                feature_3=1.0,
            ).model_dump(),
        )
    assert response.status_code == 200

    time.sleep(10)

    request = DriftRequest(
        name=drift_profile.config.name,
        space=drift_profile.config.space,
        version=drift_profile.config.version,
        time_interval=TimeInterval.FiveMinutes,
        max_data_points=100,
        drift_type=DriftType.Spc,
    )

    drift = scouter_client.get_binned_drift(request)

    assert drift.features.keys() == {
        "feature_0",
        "feature_1",
        "feature_2",
        "feature_3",
    }
    assert len(drift.features["feature_0"].values) == 1
