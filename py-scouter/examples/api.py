from pathlib import Path

import numpy as np
from config import config
from fastapi import FastAPI, Request
from pydantic import BaseModel
from scouter.integrations.fastapi import ScouterRouter
from scouter.logging import LoggingConfig, LogLevel, RustyLogger
from scouter.queue import DriftTransportConfig, Feature, Features

# Sets up logging for tests
RustyLogger.setup_logging(LoggingConfig(log_level=LogLevel.Debug))


class Response(BaseModel):
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


def setup_router() -> ScouterRouter:
    # setup kafka
    spc_transport = DriftTransportConfig(
        id=config.spc_id,
        config=config.kafka,
        drift_profile=None,
        drift_profile_path=Path("spc_profile.json"),
    )

    psi_transport = DriftTransportConfig(
        id=config.psi_id,
        config=config.kafka,
        drift_profile=None,
        drift_profile_path=Path("psi_profile.json"),
    )

    return ScouterRouter(transport=[spc_transport, psi_transport])


app = FastAPI(title="Example Drift App")
scouter_router = setup_router()


@scouter_router.get("/predict", response_model=Response)
async def psi_predict(request: Request) -> Response:
    payload = PredictRequest(
        feature_0=np.random.rand(),
        feature_1=np.random.rand(),
        feature_2=np.random.rand(),
        feature_3=np.random.rand(),
    )

    request.state.scouter_data = {
        config.psi_id: payload.to_features(),
        config.spc_id: payload.to_features(),
    }

    return Response(message="success")


app.include_router(scouter_router)
