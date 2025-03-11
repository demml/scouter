This guide will help you quickly set up PSI in Scouter with a complete end-to-end example. We'll walk through:

- Setting up a PSI drift profile
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
To detect model drift, we first need to create a drift profile using your baseline dataset $Y_{b}$
```python
from scouter.alert import PsiAlertConfig, SlackDispatchConfig
from scouter.client import ScouterClient
from scouter.drift import Drifter, PsiDriftConfig
from scouter.types import CommonCrons
from sklearn import datasets

if __name__ == "__main__":
    # Prepare data
    X, y = datasets.load_wine(return_X_y=True, as_frame=True)

    # Drifter class for creating drift profiles
    scouter = Drifter()

    # Specify the alert configuration
    alert_config = PsiAlertConfig(
        # When scouter server runs a drift detection job, what features should be analyzed
        features_to_monitor=["malic_acid", "total_phenols", "color_intensity"],
        # I want scouter server to check for drift every day (use CommonCrons or a custom cron string)
        schedule=CommonCrons.EveryDay,
        # If scouter server detects drift, notify me viaSlack. Scouter server also supports Opsgenie notifications!
        dispatch_config=SlackDispatchConfig(channel="test_channel"),
        # Leave the default PSI threshold of 0.25, or adjust as needed. For details, refer to the PSI theory section in the docs.
        # psi_threshold=0.25  # (default)
    )

    # Create the drift config, used for versioning and housing the alert config
    psi_config = PsiDriftConfig(
        name="wine_model",
        repository="wine_model",
        version="0.0.1",
        alert_config=alert_config
    )

    # Create the drift profile
    psi_profile = scouter.create_drift_profile(X, psi_config)

    # Instantiate a Scouter client to interact with the Scouter server
    client = ScouterClient()

    # Register your profile with scouter server, set_active must be set to true if you want scouter server to run the drift detection job
    client.register_profile(profile=psi_profile, set_active=True)
```


### Scouter Queues and FastAPI Integration

At ths point we have registered a PSI drift profile with scouter server. Our profile configuration included a schedule, this will instruct scouter to run a drift detection once a day.
At this point, we have yet to collect any target data, i.e. $Y,$ and without the target data, we have nothing to compare. In the example below, we will obtain our target data
by simulating a production scenario where a client sends requests to your API service to perform inference on your model. For this demonstration, we’ll use FastAPI, as
Scouter provides a custom router that simplifies and optimizes the use of Scouter queues. If you’re not using FastAPI, refer to the Scouter queues documentation for a more general implementation.

```python
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
                name="wine_model",
                repository="wine_model",
                version="0.0.1",
                drift_type=DriftType.Psi
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
```
PSI queues are configured to send data to the server either when they reach a count of 1000 or after 30 seconds have passed, whichever comes first.

### Detecting drift and being alerted
Now that we have both our base $Y_{b}$ and target $Y$ data, scouter server will run the drift detection job and alert us via Slack if needed.

## Next Steps

- Check out the **Configuration Guide** for advanced options
- Learn more about **Drift Detection Methods**
