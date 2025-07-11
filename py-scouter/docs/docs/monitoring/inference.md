After you've saved and registered your profile with the Scouter Server, you're all set to go with real-time model monitoring. All you need is your **profile**, a **ScouterQueue** and some data to send. Below is a simple example showing how you can integrate Scouter into a `FastAPI` application.


## Loading your Profile

It is expected that you have already created and registered your profile with the Scouter Server. In addition, your profile should either be saved or downloaded to a local path. The ScouterQueue will load the profile from the local path and use it to setup the background queue and producer.

### Setting up the ScouterQueue

In this step we will attach the ScouterQueue to the FastAPI app state via the lifespan

```python
from contextlib import asynccontextmanager

from fastapi import FastAPI
from pydantic import BaseModel
from scouter import ScouterQueue, HTTPConfig


@asynccontextmanager
async def lifespan(app: FastAPI):
    app.state.queue = ScouterQueue.from_path( #(1)
        path={"psi": profile_path},
        transport_config=HTTPConfig(), #(2)
    )
    yield
    # Shutdown the queue
    fast_app.state.queue.shutdown() #(3)
    fast_app.state.queue = None

app = FastAPI(lifespan=lifespan)
```

1. The ScouterQueue `from_path` staticmethod expect's a dictionary of paths where keys are aliases and values are paths to the local profile. 
2. The transport config is used to setup the specific transport producer for the queue (kafka, rabbitmq, etc.). In this case we are using the HTTP transport config.
3. The shutdown method will stop the background queue and producer. It is important to call this method when the application is shutting down to ensure that all events are processed and sent to the Scouter server.


### Available Transport Configs

In addition to the HTTP transport config, Scouter also support the following transport/producers:

- **Kafka**: `from_path(path, transport_config=KafkaConfig())`
- **RabbitMQ**: `from_path(path, transport_config=RabbitMQConfig())`
- **Redis**: `from_path(path, transport_config=RedisConfig())`

For more information on how to configure these transports, please refer to the [queue](../api/queue.md) documentation and the server documentation.

### Inserting data

There are a variety of ways in which you can configure your api to send data. In this case, we're going to keep it simple and add the insertion logic in with the api prediction route.


```python
from fastapi import FastAPI
from pydantic import BaseModel
from scouter import Features, Feature


class Response(BaseModel):
    value: float


class PredictRequest(BaseModel):
    feature_1: float
    feature_2: float
    feature_3: float

    def to_features(self, prediction: float) -> Features:  # (1)
        model = self.model_dump()
        model["prediction"] = prediction
        return Features(features=model)


app = FastAPI(lifespan=lifespan)

@app.post("/predict", response_model=Response)
async def predict(request: Request, payload: PredictRequest) -> Response:
    #... existing prediction logic
    request.app.state.queue["psi"].insert(payload.to_features()) #(2)
    return Response(value=prediction)
```

1. The queue expects either a `Features` object or a `Metrics` object (when inserting custom metrics). In this case we are manually implementing the features logic by passing a dictionary to the `Features` class. There are a variety of ways to create features, which are shown in the `Feature` docstring. You can also use the `FeatureMixin` to automatically convert a Pydantic model to features.
```python
from scouter import FeatureMixin
class PredictRequest(FeatureMixin, BaseModel):
    feature_1: float
    feature_2: float
    feature_3: float

request.to_features()
```
2. Access the queue from the app state using the assigned alias and insert

In the above logic, we access the queue via the request.app.state and the corresponding alias. We then call the insert method with the features we want to send to the Scouter server. This is a simple exchange of data, as the ScouterQueue will pass the features through a channel to a background worker that is running independently on a separate thread. In our benchmarks, inserting data is extremely fast (<1us), so you can expect minimal overhead in your API response time. However, if you want to move the insertion logic to a background task, you can use the `BackgroundTasks` from FastAPI to do so.


### Ready to go!

And that's all you need to get started for real-time model monitoring with Scouter. For more technical discussion on the ScouterQueue, please refer to the [ScouterQueue](../specs/ts-component-scouter-queue.md) documentation.