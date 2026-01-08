After you've saved and registered your profile with the Scouter Server, you're all set to go with real-time model monitoring. All you need is your **profile**, a **ScouterQueue** and some data to send. Below is a simple example showing how you can integrate Scouter into a `FastAPI` application.


## Loading your Profile

It is expected that you have already created and registered your profile with the Scouter Server. In addition, your profile should either be saved or downloaded to a local path. The ScouterQueue will load the profile from the local path and use it to setup the background queue and producer.

## Setting up the ScouterQueue

In this step we will attach the ScouterQueue to the FastAPI app state via the lifespan

```python
from contextlib import asynccontextmanager

from fastapi import FastAPI
from pydantic import BaseModel
from scouter import ScouterQueue, HttpConfig


@asynccontextmanager
async def lifespan(app: FastAPI):
    app.state.queue = ScouterQueue.from_path( #(1)
        path={"psi": profile_path},
        transport_config=HttpConfig(), #(2)
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


## Available Transport Configs

In addition to the HTTP transport config, Scouter also support the following transport/producers:

- **Kafka**: `from_path(path, transport_config=KafkaConfig())`
- **RabbitMQ**: `from_path(path, transport_config=RabbitMQConfig())`
- **Redis**: `from_path(path, transport_config=RedisConfig())`

For more information on how to configure these transports, please refer to the [queue](/scouter/docs/api/scouter/) documentation and the server documentation.

## Inserting data

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

    def to_features(self, target: float) -> Features:  # (1)
        model = self.model_dump()
        model["target"] = target
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

In the above logic, we access the queue via the `request.app.state` and the corresponding alias ("psi"). We then call the insert method with the features we want to send to the Scouter server. This is a simple exchange of data, as the ScouterQueue will pass the features through a channel to a background worker that is running independently on a separate thread. In our benchmarks, inserting data is extremely fast (<1us), so you can expect minimal overhead in your API response time. However, if you want to move the insertion logic to a background task, you can use the `BackgroundTasks` from FastAPI to do so.

### What Queues Expect
As you can see in the above example, the `ScouterQueue` expects either a `Features` object, a `Metrics` object or an `GenAIEvalRecord` object. Both of these objects are designed to be flexible and can be created in a variety of ways.

### When to use `Features` vs `Metrics` vs `GenAIEvalRecord`?

| `type`  | `Description` | `Associated Profiles` |
|---------|----------------|-----------------------|
| `Features` | Used for PSI and SPC monitoring, where you are monitoring 'features' | `PsiDriftProfile`, `SpcDriftProfile` |
| `Metrics` | Used for custom metrics that you want to monitor | `CustomMetricProfile` |
| `GenAIEvalRecord` | Used for LLM monitoring, where you are monitoring the performance of LLM services | `GenAIEvalProfile` |

### How to create `Features`, `Metrics` and `GenAIEvalRecord` objects?

#### Features

The `Features` object can be created from a dictionary of key-value pairs, where the keys are the feature names (string) and the values are the feature values (float, int, string). **Note** - these types should correspond to the types that were inferred while creating a drift profile (i.e. if `feat1` was inferred as a `float`, then you should pass a `float` value for `feat1` when inserting data). You can also create a `Features` object by passing a list of `Feature` objects, where each `Feature` object represents a single feature with a name and value.

**Using a list of features**

```python
from scouter.queue import Features, Feature
# Passing a list of features
features = Features(
    features=[
        Feature("feature_1", 1),
        Feature("feature_2", 2.0),
        Feature("feature_3", "value"),
    ]
)
```

**Using a dictionary (pydantic model)**

```python

# Passing a dictionary (pydantic model) of features
class MyFeatures(BaseModel):
    feature1: int
    feature2: float
    feature3: str

my_features = MyFeatures(
    feature1=1,
    feature2=2.0,
    feature3="value",
)

features = Features(my_features.model_dump())
```

**Using a FeatureMixin**

`Scouter` also comes with a `FeatureMixin`, that can be used to automatically convert a Pydantic model to a `Features` object. This is useful when you want to send the entire model as features without manually creating the `Features` object

```python
from scouter.util import FeatureMixin

class MyFeatures(FeatureMixin, BaseModel):
    feature1: int
    feature2: float
    feature3: str

my_features = MyFeatures(
    feature1=1,
    feature2=2.0,
    feature3="value",
)

features = my_features.to_features()
```

#### Metrics

`Metrics` also follow a similar pattern to `Features`.

**Using a list of metrics**

```python
from scouter.queue import Metrics, Metric

# Supply a list of metrics
Metrics(
    [
        Metric("metric_1", 1),
        Metric("metric_2", 2.0),
    ]
)
```

**Using a dictionary (pydantic model)**

When using a dictionary, the key should match the metric name in your profile.

```python
from scouter.queue import Metrics, Metric
from pydantic import BaseModel

class MyMetrics(BaseModel):
    mae: int
    mape: float

my_metrics = MyMetrics(mae=1, mape=2.0)
Metrics(my_metrics.model_dump())
```


#### GenAIEvalRecord
The `GenAIEvalRecord` object is used to send LLM records to the Scouter server for monitoring. It contains the input, response, and context of the LLM service. You can create an `GenAIEvalRecord` object by passing the input, response, and context as parameters.

**Note**

 Input, response and context should be serializable types (i.e. strings, numbers, lists, dictionaries). If you want to send more complex objects, you can use the `to_json` method to convert them to a JSON string. In addition, all of these fields will be injected into your LLM metric prompts on the server side, so if your prompts expect `${input}` or `${response}`, you can use these fields to populate them.

```python

record = GenAIEvalRecord(
    input="What is the capital of France?",
    response="Paris is the capital of France.",
    context={"foo": "bar"}
)
```

GenAIEvalRecord Arguments

| `Argument` | `Type` | `Description` |
|------------|--------|----------------|
| `input` | `str | int | float | dict | list` | The input to the LLM service. This could be something like a user question |
| `response` | `str | int | float | dict | list` | The response from the LLM service. This could be something like the answer to the user question |
| `context` | `Dict[str, Any]` | The context for the LLM service (if any). The keys should map to the context variables in your LLM metric prompts |
| `prompt` | `Prompt str | int | float | dict | list` | The prompt used to generate the response |

### Ready to go!

And that's all you need to get started for real-time model monitoring with Scouter. For more technical discussion on the ScouterQueue, please refer to the [ScouterQueue](/scouter/docs/specs/ts-component-scouter-queue/) documentation.