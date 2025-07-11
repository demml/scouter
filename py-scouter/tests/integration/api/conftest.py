from contextlib import asynccontextmanager
from pathlib import Path

import numpy as np
import pandas as pd
from fastapi import FastAPI, Request
from pydantic import BaseModel
from scouter import HTTPConfig, KafkaConfig, ScouterQueue  # type: ignore[attr-defined]
from scouter.alert import SpcAlertConfig
from scouter.client import ScouterClient
from scouter.drift import Drifter, SpcDriftConfig, SpcDriftProfile
from scouter.logging import LoggingConfig, LogLevel, RustyLogger
from scouter.util import FeatureMixin

logger = RustyLogger.get_logger(
    LoggingConfig(log_level=LogLevel.Debug),
)


def generate_data() -> pd.DataFrame:
    """Create a fake data frame for testing"""
    n = 10_000

    X_train = np.random.normal(-4, 2.0, size=(n, 4))

    col_names = []
    for i in range(0, X_train.shape[1]):
        col_names.append(f"feature_{i}")

    X = pd.DataFrame(X_train, columns=col_names)

    return X


def create_and_register_drift_profile(
    client: ScouterClient,
    name: str,
) -> SpcDriftProfile:
    data = generate_data()

    # create drift config (usually associated with a model name, space name, version)
    config = SpcDriftConfig(
        space="scouter",
        name=name,
        version="0.1.0",
        alert_config=SpcAlertConfig(features_to_monitor=data.columns.tolist()),
    )

    # create drifter
    drifter = Drifter()

    # create drift profile
    profile = drifter.create_drift_profile(data, config)
    client.register_profile(profile, True)

    return profile


class TestResponse(BaseModel):
    message: str


class PredictRequest(BaseModel, FeatureMixin):
    feature_0: float
    feature_1: float
    feature_2: float
    feature_3: float


def create_kafka_app(profile_path: Path) -> FastAPI:
    config = KafkaConfig()

    @asynccontextmanager
    async def lifespan(app: FastAPI):
        logger.info("Starting up FastAPI app")

        app.state.queue = ScouterQueue.from_path(
            path={"spc": profile_path},
            transport_config=config,
        )
        yield

        logger.info("Shutting down FastAPI app")
        # Shutdown the queue
        app.state.queue.shutdown()
        app.state.queue = None

    app = FastAPI(lifespan=lifespan)

    @app.post("/predict", response_model=TestResponse)
    async def predict(request: Request, payload: PredictRequest) -> TestResponse:
        print(f"Received payload: {request.app.state}")
        request.app.state.queue["spc"].insert(payload.to_features())
        return TestResponse(message="success")

    return app


def create_http_app(profile_path: Path) -> FastAPI:
    config = HTTPConfig()

    @asynccontextmanager
    async def lifespan(app: FastAPI):
        logger.info("Starting up FastAPI app")

        app.state.queue = ScouterQueue.from_path(
            path={"spc": profile_path},
            transport_config=config,
        )
        yield

        logger.info("Shutting down FastAPI app")
        # Shutdown the queue
        app.state.queue.shutdown()
        app.state.queue = None

    app = FastAPI(lifespan=lifespan)

    @app.post("/predict", response_model=TestResponse)
    async def predict(request: Request, payload: PredictRequest) -> TestResponse:
        print(f"Received payload: {request.app.state}")
        request.app.state.queue["spc"].insert(payload.to_features())
        return TestResponse(message="success")

    return app
