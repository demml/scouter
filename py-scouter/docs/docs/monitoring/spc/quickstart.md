This guide will help you quickly set up SPC in Scouter with a complete end-to-end example. We'll walk through:

- Setting up a SPC drift profile
- Configuring real-time notifications for model drift detection
- Using scouter queues and fastapi integrations allowing you to send data to scouter server at the time of model inference

### Installation

```bash
pip install scouter
```

### **Configuration**
To register profiles and use Scouter queues, set your company's Scouter server URL as an environment variable:

```bash
export SCOUTER_SERVER_URL=your_scouter_server_url
```

### Creating a Drift Profile
To detect model drift, we first need to create a drift profile using your training data, but before doing that we will define a custom SPC alert rule.




```python hl_lines="6 15"
from scouter.alert import SlackDispatchConfig, SpcAlertConfig, SpcAlertRule
from scouter.client import ScouterClient
from scouter.drift import Drifter, SpcDriftConfig
from scouter.types import CommonCrons
from sklearn import datasets

if __name__ == "__main__":
    # Prepare data
    X, y = datasets.load_wine(return_X_y=True, as_frame=True)

    # Drifter class to create drift profiles
    scouter = Drifter()

    # Specify the alert configuration
    alert_config = SpcAlertConfig(
        features_to_monitor=["malic_acid", "total_phenols", "color_intensity"], # Defaults to all features if left empty
        schedule=CommonCrons.EveryDay, # Run drift detection job once daily
        dispatch_config=SlackDispatchConfig(channel="test_channel"), # Notify my team Slack channel if drift is detected
        rule=SpcAlertRule(rule="16 32 4 8 2 4 1 1"), # See the spc theory doc for additional info
    )

    # Set up SPC drift config with a custom sample size
    spc_config = SpcDriftConfig(name="wine_model", space="wine_model", version="0.0.1", alert_config=alert_config, sample_size=1000)

    # Create the drift profile
    spc_profile = scouter.create_drift_profile(X, spc_config)

    # Register your profile with scouter server
    client = ScouterClient()
    # set_active must be set to True if you want scouter server to run the drift detection job
    client.register_profile(profile=spc_profile, set_active=True)
```


### Scouter Queues and FastAPI Integration

At ths point we have registered a SPC drift profile with scouter server. Our profile configuration included a schedule, this will instruct scouter to run a drift detection once a day.
At this point, we have yet to collect any observed data, and without observed data, we have nothing to compare. In the example below, we will obtain our observed data
by simulating a production scenario where a client sends requests to your API service to perform inference on your model. For this demonstration, we’ll use FastAPI, as
Scouter provides a custom router that simplifies and optimizes the use of Scouter queues. If you’re not using FastAPI, refer to the Scouter queues documentation for a more general implementation.

```python
from fastapi import FastAPI, Request
from pydantic import BaseModel
from scouter.client import GetProfileRequest
from scouter.integrations.fastapi import ScouterRouter
from scouter.queue import DriftTransportConfig, Feature, Features, KafkaConfig
from scouter.types import DriftType

# Unique ID for Scouter queue, useful if using multiple drift types (e.g., SPC and PSI)
DRIFT_TRANSPORT_QUEUE_ID = "spc_id"


class Response(BaseModel):
    message: str

class PredictRequest(BaseModel):
    malic_acid: float
    total_phenols: float
    color_intensity: float

    # This helper function is necessary to convert Scouter Python types into the appropriate Rust types
    def to_features(self) -> Features:
        return Features(
            features=[
                Feature.float("malic_acid", self.malic_acid),
                Feature.float("total_phenols", self.total_phenols),
                Feature.float("color_intensity", self.color_intensity),
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
                space="wine_model",
                version="0.0.1",
                drift_type=DriftType.Spc
            ),
        )
    ]
)

# Use the Scouter router to handle prediction post requests
@scouter_router.post("/predict", response_model=Response)
async def spc_predict(request: Request, payload: PredictRequest) -> Response:
    # Store transformed features in the request state under 'scouter_data' for the specified queue ID
    request.state.scouter_data = {
        DRIFT_TRANSPORT_QUEUE_ID: payload.to_features(),
    }
    return Response(message="success")



app = FastAPI(title="Example Drift App")
# Register the scouter router
app.include_router(scouter_router)
```
SPC queues use your `SpcDriftConfig` and the specified sample_size to decide when to send queue data to the scouter server.

### Detecting drift and being alerted
Now that we have both our base and observed data, scouter server will run the drift detection job and alert us via Slack if needed.

## Next Steps

- Check out the additional SPC configuration guides for more details.
