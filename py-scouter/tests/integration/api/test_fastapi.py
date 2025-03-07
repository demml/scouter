import time

from fastapi.testclient import TestClient
from scouter.client import (
    BinnedSpcFeatureMetrics,
    DriftRequest,
    ScouterClient,
    TimeInterval,
)
from scouter.drift import SpcDriftProfile
from scouter.logging import LoggingConfig, LogLevel, RustyLogger
from scouter.types import DriftType

from tests.integration.api.conftest import PredictRequest

RustyLogger.setup_logging(
    LoggingConfig(log_level=LogLevel.Debug),
)


def test_router_mixin_kafka(
    drift_profile: SpcDriftProfile,
    client: TestClient,
):

    scouter_client = ScouterClient()
    for i in range(30):
        response = client.post(
            "/predict",
            json=PredictRequest(
                feature_0=1.0,
                feature_1=1.0,
                feature_2=1.0,
                feature_3=1.0,
            ).model_dump(),
        )
        breakpoint()
    assert response.status_code == 200
    time.sleep(2)
    breakpoint()
    drift: BinnedSpcFeatureMetrics = scouter_client.get_binned_drift(
        DriftRequest(
            name=drift_profile.config.name,
            repository=drift_profile.config.repository,
            version=drift_profile.config.version,
            time_interval=TimeInterval.FiveMinutes,
            max_data_points=100,
            drift_type=DriftType.Spc,
        )
    )
    breakpoint()
    assert drift.features.keys() == {"feature_0", "feature_1", "feature_2", "feature_3"}
    assert len(drift.features["feature_0"].values) == 1
