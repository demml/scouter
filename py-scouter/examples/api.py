from pathlib import Path

import numpy as np
from fastapi import FastAPI, Request
from pydantic import BaseModel
from scouter.drift import PsiDriftProfile, SpcDriftProfile
from scouter.integrations.fastapi import ScouterRouter
from scouter.logging import LoggingConfig, LogLevel, RustyLogger
from scouter.queue import Feature, Features, KafkaConfig

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


def setup_psi_router() -> ScouterRouter:
    # setup kafka
    config = KafkaConfig()
    return ScouterRouter(
        drift_profile=PsiDriftProfile.from_file(Path("psi_profile.json")),
        config=config,
    )
    
def setup_spc_router() -> ScouterRouter:
    # setup kafka
    config = KafkaConfig()
    return ScouterRouter(
        drift_profile=SpcDriftProfile.from_file(Path("spc_profile.json")),
        config=config,
    )


app = FastAPI(title="Example Drift App")
psi_router = setup_psi_router()
spc_router = setup_spc_router()



@psi_router.get("/psi_predict", response_model=Response)
async def psi_predict(request: Request) -> Response:
    payload = PredictRequest(
        feature_0=np.random.rand(),
        feature_1=np.random.rand(),
        feature_2=np.random.rand(),
        feature_3=np.random.rand(),
    )
    request.state.scouter_data = payload.to_features()
    return Response(message="success")

@spc_router.get("/spc_predict", response_model=Response)
async def spc_predict(request: Request) -> Response:
    payload = PredictRequest(
        feature_0=np.random.rand(),
        feature_1=np.random.rand(),
        feature_2=np.random.rand(),
        feature_3=np.random.rand(),
    )
    request.state.scouter_data = payload.to_features()
    return Response(message="success")


app.include_router(psi_router)
app.include_router(spc_router)
