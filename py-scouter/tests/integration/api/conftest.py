import numpy as np
import pandas as pd
import pytest
from fastapi import FastAPI, Request
from fastapi.testclient import TestClient
from pydantic import BaseModel
from scouter.drift import Drifter, SpcDriftConfig, SpcDriftProfile
from scouter.integrations.fastapi import ScouterRouter
from scouter.queue import Feature, Features, KafkaConfig


def generate_data() -> pd.DataFrame:
    """Create a fake data frame for testing"""
    n = 10_00

    X_train = np.random.normal(-4, 2.0, size=(n, 4))

    col_names = []
    for i in range(0, X_train.shape[1]):
        col_names.append(f"feature_{i}")

    X = pd.DataFrame(X_train, columns=col_names)

    return X


@pytest.fixture
def drift_profile():
    data = generate_data()

    # create drift config (usually associated with a model name, repository name, version)
    config = SpcDriftConfig(repository="scouter", name="model", version="0.1.0")

    # create drifter
    drifter = Drifter()

    # create drift profile
    profile = drifter.create_drift_profile(data, config)

    return profile


class TestResponse(BaseModel):
    message: str


class PredictRequest(BaseModel):
    feature_0: float
    feature_1: float
    feature_2: float
    feature_3: float

    def to_features(self) -> Features:
        return Features(
            features=[
                Feature.float("feature_0", self.feature_0),
                Feature.float("feature_1", self.feature_1),
                Feature.float("feature_2", self.feature_2),
                Feature.float("feature_3", self.feature_3),
            ]
        )


@pytest.fixture
def client(
    drift_profile: SpcDriftProfile,
) -> TestClient:

    config = KafkaConfig()
    app = FastAPI()

    # define scouter router
    scouter_router = ScouterRouter(drift_profile=drift_profile, config=config)

    @scouter_router.post("/predict", response_model=TestResponse)
    async def predict(request: Request, payload: PredictRequest) -> TestResponse:
        request.state.scouter_data = payload.to_features()
        return TestResponse(message="success")

    app.include_router(scouter_router)

    return TestClient(app)
