from fastapi import FastAPI, Request
from scouter.queue import KafkaConfig, Feature, Features
from scouter.integrations.fastapi import ScouterRouter
from scouter.drift import SpcDriftProfile, PsiDriftProfile
from scouter.logging import RustyLogger, LoggingConfig, LogLevel
from pydantic import BaseModel
import numpy as np

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
    config = KafkaConfig()
    
    # load profile 
    # need to add this method to the PsiDriftProfile class
    # PsiDriftProfile.from_file("examples/psi_drift_profile.json")
    with open("drift_profile.json", "r") as f:
       drift_profile =  PsiDriftProfile.model_validate_json(f.read())
    
    
    return  ScouterRouter(drift_profile=drift_profile, config=config)
    
    
app = FastAPI(title="Example Drift App")
scouter_router = setup_router()

@scouter_router.get("/predict", response_model=Response)
async def predict(request: Request) -> Response:
    payload = PredictRequest(
        feature_0=np.random.rand(),
        feature_1=np.random.rand(),
        feature_2=np.random.rand(),
        feature_3=np.random.rand(),
    )
    request.state.scouter_data = payload.to_features()
    return Response(message="success")

app.include_router(scouter_router)