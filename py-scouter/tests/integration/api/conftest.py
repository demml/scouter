import numpy as np
import pandas as pd
from fastapi import FastAPI, Request
from pydantic import BaseModel
from scouter.alert import SpcAlertConfig
from scouter.client import GetProfileRequest, ScouterClient
from scouter.drift import Drifter, SpcDriftConfig, SpcDriftProfile
from scouter import Feature, Features, KafkaConfig, ScouterQueue
from pathlib import Path
import tempfile
from typing import Generator
from contextlib import asynccontextmanager


def generate_data() -> pd.DataFrame:
    """Create a fake data frame for testing"""
    n = 10_000

    X_train = np.random.normal(-4, 2.0, size=(n, 4))

    col_names = []
    for i in range(0, X_train.shape[1]):
        col_names.append(f"feature_{i}")

    X = pd.DataFrame(X_train, columns=col_names)

    return X


def create_and_register_drift_profile() -> Generator[Path, None, None]:
    data = generate_data()

    # create drift config (usually associated with a model name, space name, version)
    config = SpcDriftConfig(
        space="scouter",
        name="model",
        version="0.1.0",
        alert_config=SpcAlertConfig(features_to_monitor=data.columns.tolist()),
    )

    # create drifter
    drifter = Drifter()

    # create drift profile
    profile = drifter.create_drift_profile(data, config)

    # register the profile so we can obtain it during our drift transport configuration
    # save profile to json in a temporary directory
    with tempfile.TemporaryDirectory() as temp_dir:
        path = Path(temp_dir) / "profile.json"
        profile.save_to_json(path)
        assert (Path(temp_dir) / "profile.json").exists()

        yield path


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


def create_app(profile_path: Path) -> FastAPI:
    config = KafkaConfig()
    app = FastAPI()

    @asynccontextmanager
    async def lifespan(app: FastAPI):
        # Load the ML model
        app.state.scouter_queue = ScouterQueue(
            path={"profile": profile_path},
            transport_config=config,
        )
        yield

    async def predict(request: Request, payload: PredictRequest) -> TestResponse:
        request.state.scouter_data = payload.to_features()
        request.state.scouter_queue["profile"].insert(payload.to_features())

        return TestResponse(message="success")

    app.include_router(scouter_router)

    return app
