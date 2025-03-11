from fastapi import FastAPI, Request
from pydantic import BaseModel
from scouter.client import GetProfileRequest
from scouter.integrations.fastapi import ScouterRouter
from scouter.queue import DriftTransportConfig, Feature, Features, KafkaConfig
from scouter.types import DriftType

# Each Scouter queue requires a unique user-defined ID.
# This ensures proper tracking of multiple drift profile types,
# allowing the FastAPI integration to associate each queue with the correct profile type.
DRIFT_TRANSPORT_QUEUE_ID = "psi_id"


# Simple response model for our post request.
class Response(BaseModel):
    message: str


# Simple reqeust model for our post request.
class PredictRequest(BaseModel):
    malic_acid: float
    total_phenols: float
    color_intensity: float

    # This helper function is necessary to convert Scouter Python types into the appropriate Rust types.
    def to_features(self) -> Features:
        return Features(
            features=[
                Feature.float("malic_acid", self.malic_acid),
                Feature.float("total_phenols", self.total_phenols),
                Feature.float("color_intensity", self.color_intensity),
            ]
        )


# We use ScouterRouter, a custom extension of FastAPI's APIRouter,
# to integrate seamlessly with FastAPI's background task system.
# This enables Scouter to manage queue updates efficiently, ensuring that all queue processes
# are handled as FastAPI background tasks.
scouter_router = ScouterRouter(
    # We pass a single drift transport configuration here. If working with multiple drift profile types,
    # we can easily extend this list with additional configurations.
    transport=[
        DriftTransportConfig(
            id=DRIFT_TRANSPORT_QUEUE_ID,
            # Kafka is chosen as the transport mode, but RabbitMQ is also supported.
            # To use Kafka, ensure both KAFKA_BROKERS and KAFKA_TOPIC environment variables are set.
            config=KafkaConfig(),
            # Drift transport configurations are tied to drift profiles. The drift_profile_request specifies
            # which profile the Scouter client should fetch from the server.
            drift_profile_request=GetProfileRequest(
                name="wine_model", repository="wine_model", version="0.0.1", drift_type=DriftType.Psi
            ),
        )
    ]
)


# Use the Scouter router to handle prediction post requests
@scouter_router.post("/predict", response_model=Response)
async def psi_predict(request: Request, payload: PredictRequest) -> Response:
    # The FastAPI Scouter integration expects queue data to be stored in the request state, under 'scouter_data'.
    # Here, we construct a dictionary where the queue ID is the key and the payload's transformed features are the value.
    request.state.scouter_data = {
        DRIFT_TRANSPORT_QUEUE_ID: payload.to_features(),
    }
    return Response(message="success")


app = FastAPI(title="Example Drift App")
# Register the scouter router
app.include_router(scouter_router)
