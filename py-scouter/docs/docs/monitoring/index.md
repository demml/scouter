Out of the box, Scouter provides functionality to create drift profiles and perform real-time monitoring. 

## Drift Profiles

Drift profiles are created using the `Drifter` class, which provides a simple interface for creating and managing drift profiles. The `Drifter` class supports multiple drift detection methods, including:
- **Population Stability Index (PSI)** – A standard approach for detecting distribution shifts.
- **Statistical Process Control (SPC)** – A proven method widely used in manufacturing and operations.
- **Custom Metrics** – Define your own drift detection method to match your specific needs.

## Scouter Queues
Scouter Queues allow you to capture the data being sent to your model during inference. This captured data is then sent to the Scouter server, where it will be stored and used in the future to detect any potential drift and notify you and your team based on your configuration.

## Alerting
Based on the drift profile you configure, a scheduled job periodically checks for data drift using the captured inference data. If drift is detected, an alert is triggered and sent via your preferred method to notify the relevant team.

## Supported Data Types
Scouter supports a variety of data types, including:
- **Pandas DataFrames**: Scouter can handle Pandas DataFrames, making it easy to integrate with existing data processing pipelines.
- **Numpy Arrays**: Out of the box support for 2D arrays.
- **Polars DataFrames**: For users who prefer Polars, Scouter supports this data format as well, allowing for efficient data processing and analysis.
- **Custom Metrics**: Scouter allows you to define your own custom metrics for drift detection, giving you the flexibility to tailor the monitoring process to your specific needs.

## Getting Started (Client Quickstart)

### **Installation**

```bash
pip install scouter-ml
```

### **Configuration**
To register profiles and use Scouter queues, set the Scouter server URI as an environment variable:

```bash
export SCOUTER_SERVER_URI=your_SCOUTER_SERVER_URI
```

### Creating a Drift Profile

To detect model drift, we first need to create a drift profile using your baseline dataset, this is typically done at the time of training your model.

The following example is taken directly from the examples/psi/api.py file in the Scouter repository. It demonstrates how to create a drift profile using the `Drifter` class and register it with the Scouter server and then use the Scouter queues to send data to the Scouter server for drift detection.

```python
import numpy as np
import pandas as pd
import uvicorn
from fastapi import FastAPI, Request
from pydantic import BaseModel
from scouter import (
    CommonCrons,
    Drifter,
    HTTPConfig,
    PsiAlertConfig,
    PsiDriftConfig,
    ScouterClient,
    ScouterQueue,
)
from scouter.util import FeatureMixin

class Response(BaseModel):
    message: str


class PredictRequest(BaseModel, FeatureMixin): #(1)
    feature_1: float
    feature_2: float
    feature_3: float


def generate_data() -> pd.DataFrame:
    """Create a fake data frame for testing"""
    n = 10_000
    X_train = np.random.normal(-4, 2.0, size=(n, 4))
    col_names = []
    for i in range(0, X_train.shape[1]):
        col_names.append(f"feature_{i}")
    X = pd.DataFrame(X_train, columns=col_names)
    return X


def create_psi_profile() -> Path:
    """Create a PSI profile

    The following example shows how to:

    1. Instantiate the Drifter class and connect to the Scouter client
    2. Create a fake dataframe
    3. Create a PSI profile using the Drifter class
    4. Register the profile with the Scouter client and set it as active
    (this will tell the server to schedule the profile for alerting)
    5. Save the profile to a json file (we'll use this to load it in the api for demo purposes)
    """
    # Drifter class for creating drift profiles
    drifter = Drifter() #(2)

    # Simple client to register drift profiles (scouter client must be running)
    client = ScouterClient() #(3)

    # create fake data
    data = generate_data()

    # create psi configuration
    psi_config = PsiDriftConfig( #(4)
        space="scouter",
        name="psi_test",
        version="0.0.1",
        alert_config=PsiAlertConfig(
            schedule=CommonCrons.Every6Hours,
            features_to_monitor=[
                "feature_1",
                "feature_2",
            ],
        ),
    )

    # create psi profile
    psi_profile = drifter.create_drift_profile(data, psi_config)

    # register profile
    client.register_profile(profile=psi_profile, set_active=True) #(5)

    # save profile to json (for example purposes)
    return psi_profile.save_to_json()


if __name__ == "__main__":
    # Create a PSI profile and get its path
    profile_path = create_psi_profile()


    # Setup api lifespan
    @asynccontextmanager
    async def lifespan(fast_app: FastAPI):

        fast_app.state.queue = ScouterQueue.from_path( #(6)
            path={"psi": profile_path},
            transport_config=HttpConfig(), #(7)
        )
        yield

        # Shutdown the queue
        fast_app.state.queue.shutdown()
        fast_app.state.queue = None

    app = FastAPI(lifespan=lifespan)

    @app.post("/predict", response_model=Response)
    async def predict(request: Request, payload: PredictRequest) -> Response:
        request.app.state.queue["psi"].insert(payload.to_features()) #(8)
        return Response(message="success")

    uvicorn.run(app, host="0.0.0.0", port=8888)
```

1. The `FeatureMixin` class is used to convert the input data into a `Features` object that is inserted into the Scouter queue. You can also do this manually by using the `Feature` and `Features` classes directly. Refer to the [Scouter Queues](#) section for more information
2. The `Drifter` class is used to create drift profiles
3. The `ScouterClient` class is used to register the drift profile with the Scouter server
4. `DriftConfig` is a required argument to all drift types. It helps define how the drift profile is created and how the drift detection job is scheduled. Refer to the [DriftConfig](#) section for more information
5. The `register_profile` method is used to register the drift profile with the Scouter server. The `set_active` argument tells the server to schedule the drift detection job based on the configuration in the `DriftConfig` object. If set to `False`, the profile will not be scheduled for drift detection. You can always set this to true later
6. Here we setup the ScouterQueue within our FastApi lifespan and attach it to the application's state. You can load and set as many queues as you like. Each profile is given an alias that you can use to access later on
7. The `HttpConfig` class sets the transport configuration to send direct HTTP requests to the Scouter server from items in the queue. In production, you may want to use a different transport configuration, such as `KafkaConfig` or `RabbitMQ`, depending on your needs.
8. Insert data into the ScouterQueue using a specific alias

!!!success
    That's it! While there's a few details to iron out, you now know how to configure real-time monitoring and alerting using Scouter. Please see refer to the rest of the documentation for more details on how to use Scouter and the Scouter server. If you have any questions, please feel free to reach out to us on Slack or create an issue on GitHub.