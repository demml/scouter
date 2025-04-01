
While Scouter comes packed with powerful drift detection tools, we understand that no single solution fits every use case. You might find yourself needing a drift detection method that isn’t natively supported—don’t worry, we’ve got you covered. Scouter provides built-in support for custom metric tracking, allowing you to define your own metrics and baseline values. We’ll handle the heavy lifting of saving inference data and detecting drift over time.

This quickstart will guide you through:

- Setting up a Custom drift profile
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
Our first step is to define the custom metric along with its initial value. Together, the metric's name and baseline value form the foundation of the custom drift profile. This initial value serves as the reference point against which future values—captured either during scheduled cron jobs or, in our case, real-time inference within a FastAPI application—will be compared to detect drift.
```python hl_lines="6 15"
from scouter.alert import (
    AlertThreshold, CustomMetricAlertConfig, OpsGenieDispatchConfig,
)
from scouter.client import ScouterClient
from scouter.drift import (
    CustomDriftProfile,
    CustomMetric,
    CustomMetricDriftConfig,
)
from scouter.types import CommonCrons


def my_custom_metric() -> float:
    return 0.03


if __name__ == "__main__":
    # Specify the alert configuration
    alert_config = CustomMetricAlertConfig(
        dispatch_config=OpsGenieDispatchConfig(team='the-ds-team'), # Notify my team via Opsgenie if drift is detected
        schedule=CommonCrons.EveryWeek # Run drift detection job once weekly
    )

    # Create drift config
    custom_config = CustomMetricDriftConfig(
        name="wine_model",
        repository="wine_model",
        version="0.0.1",
        alert_config=alert_config
    )

    # Create the drift profile
    custom_profile = CustomDriftProfile(
        config=custom_config,
        metrics=[
            CustomMetric(
                name="custom_metric",
                value=my_custom_metric(),
                # Alerts if the observed metric value exceeds the baseline.
                alert_threshold=AlertThreshold.Above,
                # If alert_threshold_value isn’t set, any increase triggers an alert.
                alert_threshold_value=0.02
            ),
        ],
    )

    # Register your profile with scouter server
    client = ScouterClient()

    # set_active must be set to True if you want scouter server to run the drift detection job
    client.register_profile(custom_profile, set_active=True)
```


### Scouter Queues and FastAPI Integration

At ths point we have registered a Custom drift profile with scouter server. Our profile configuration included a schedule, this will instruct scouter to run a drift detection once a week.
At this point, we have yet to collect any observed data, and without observed data, we have nothing to compare. In the example below, we will obtain our observed data by simulating a production scenario where a client sends requests to your API service to perform inference on your model. For this demonstration, we’ll use FastAPI, as Scouter provides a custom router that simplifies and optimizes the use of Scouter queues. If you’re not using FastAPI, refer to the Scouter queues documentation for a more general implementation.

```python
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
                Metric("custom_metric", self.malic_acid),
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
async def custom_predict(request: Request, payload: PredictRequest) -> Response:
    # Store the custom metric value in the request state under 'scouter_data' for the specified queue ID
    request.state.scouter_data = {
        DRIFT_TRANSPORT_QUEUE_ID: payload.to_metrics(),
    }
    return Response(message="success")



app = FastAPI(title="Example Drift App")
# Register the scouter router
app.include_router(scouter_router)
```
Custom queues use your `CustomMetricDriftConfig` and the specified sample_size to decide when to send queue data to the scouter server, it will default to 25 if unspecified.

### Detecting drift and being alerted
Now that we have both our base and observed data, scouter server will run the drift detection job and alert us via Opsgenie if needed.

## Next Steps

- Check out the additional Custom configuration guides for more details.
