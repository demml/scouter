from fastapi import FastAPI, Request
from pydantic import BaseModel

from scouter.client import GetProfileRequest
from scouter.integrations.fastapi import ScouterRouter
from scouter.queue import DriftTransportConfig, Metric, Metrics, KafkaConfig
from scouter.types import DriftType

DRIFT_TRANSPORT_QUEUE_ID = "custom_id"

class Response(BaseModel):
    message: str

class PredictRequest(BaseModel):
    malic_acid: float
    total_phenols: float
    color_intensity: float

    def to_metrics(self) -> Metrics:
        return Metrics(
            metrics=[
                Metric("custom_metric_name", self.malic_acid),
            ]
        )


# ScouterRouter for FastAPI integration
scouter_router = ScouterRouter(
    transport=[
        DriftTransportConfig(
            id=DRIFT_TRANSPORT_QUEUE_ID,
            # Kafka as transport mode (RabbitMQ also supported).
            # To use Kafka, ensure both KAFKA_BROKERS and KAFKA_TOPIC environment variables are set
            config=KafkaConfig(),
            # Drift transport configurations are tied to drift profiles
            drift_profile_request=GetProfileRequest(
                name="wine_model",
                repository="wine_model",
                version="0.0.1",
                drift_type=DriftType.Custom
            ),
        )
    ]
)

# Use the Scouter router to handle prediction post requests
@scouter_router.post("/predict", response_model=Response)
async def psi_predict(request: Request, payload: PredictRequest) -> Response:
    # Store transformed features in the request state under 'scouter_data' for the specified queue ID
    request.state.scouter_data = {
        DRIFT_TRANSPORT_QUEUE_ID: payload.to_metrics(),
    }
    return Response(message="success")



app = FastAPI(title="Example Drift App")
# Register the scouter router
app.include_router(scouter_router)