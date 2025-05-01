After you've saved and registered your profile with the Scouter Server, you're all set to go with real-time model monitoring. All you need is you **profile**, a **ScouterQueue** and some data to send. Below is a simple example showing how you can integrate Scouter into a `FastAPI` application.


## Loading your Profile

It is expected that you have already created and registered your profile with the Scouter Server. In addition, your profile should either be save or downloaded to a local path. The ScouterQueue will load the profile from the local path and use it to setup the background queue and producer.

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

1. The ScouterQueue `from_path` staticmethod expect's a deictionary of paths where keys are aliases and values are paths to the local profile. 
2. The transport config is used to setup the specific transport producer for the queue (kafka, rabbitmq, etc.). In this case we are using the HTTP transport config.
3. The shutdown method will stop the background queue and producer. It is important to call this method when the application is shutting down to ensure that all events are processed and sent to the Scouter server.


### Available Transport Configs

In addition to the HTTP transport config, Scouter also support the following transport/producers:

- **Kafka**: `from_path(path, transport_config=KafkaConfig())`
- **RabbitMQ**: `from_path(path, transport_config=RabbitMQConfig())`
- **Redis**: `from_path(path, transport_config=RedisConfig())`

#### KafkaConfig

For those using Kafka, the `KafkaConfig` class allows you to specify the following parameters:

???note "Config Definition"
    ```python
    class KafkaConfig:
        brokers: str
        topic: str
        compression_type: str
        message_timeout_ms: int
        message_max_bytes: int
        log_level: LogLevel
        config: Dict[str, str]
        max_retries: int
        transport_type: TransportType

        def __init__(
            self,
            brokers: Optional[str] = None,
            topic: Optional[str] = None,
            compression_type: Optional[str] = None,
            message_timeout_ms: int = 600_000,
            message_max_bytes: int = 2097164,
            log_level: LogLevel = LogLevel.Info,
            config: Dict[str, str] = {},
            max_retries: int = 3,
        ) -> None:
            """Kafka configuration to use with the KafkaProducer.

            Args:
                brokers:
                    Comma-separated list of Kafka brokers.
                    If not provided, the value of the KAFKA_BROKERS environment variable is used.

                topic:
                    Kafka topic to publish messages to.
                    If not provided, the value of the KAFKA_TOPIC environment variable is used.

                compression_type:
                    Compression type to use for messages.
                    Default is "gzip".

                message_timeout_ms:
                    Message timeout in milliseconds.
                    Default is 600_000.

                message_max_bytes:
                    Maximum message size in bytes.
                    Default is 2097164.

                log_level:
                    Log level for the Kafka producer.
                    Default is LogLevel.Info.

                config:
                    Additional Kafka configuration options. These will be passed to the Kafka producer.
                    See https://kafka.apache.org/documentation/#configuration.

                max_retries:
                    Maximum number of retries to attempt when publishing messages.
                    Default is 3.

            """
    ```

#### RabbitMQConfig
For those using RabbitMQ, the `RabbitMQConfig` class allows you to specify the following parameters:

???note "Config Definition"
    ```python
    class RabbitMQConfig:
    address: str
    queue: str
    max_retries: int
    transport_type: TransportType

    def __init__(
        self,
        host: Optional[str] = None,
        port: Optional[int] = None,
        username: Optional[str] = None,
        password: Optional[str] = None,
        queue: Optional[str] = None,
        max_retries: int = 3,
    ) -> None:
        """RabbitMQ configuration to use with the RabbitMQProducer.

        Args:
            host:
                RabbitMQ host.
                If not provided, the value of the RABBITMQ_HOST environment variable is used.

            port:
                RabbitMQ port.
                If not provided, the value of the RABBITMQ_PORT environment variable is used.

            username:
                RabbitMQ username.
                If not provided, the value of the RABBITMQ_USERNAME environment variable is used.

            password:
                RabbitMQ password.
                If not provided, the value of the RABBITMQ_PASSWORD environment variable is used.

            queue:
                RabbitMQ queue to publish messages to.
                If not provided, the value of the RABBITMQ_QUEUE environment variable is used.

            max_retries:
                Maximum number of retries to attempt when publishing messages.
                Default is 3.
        """
    ```

#### RedisConfig
For those using Redis, the `RedisConfig` class allows you to specify the following parameters:

???note "Config Definition"
    ```python
    class RedisConfig:
        address: str
        channel: str
        transport_type: TransportType

        def __init__(
            self,
            address: Optional[str] = None,
            chanel: Optional[str] = None,
        ) -> None:
            """Redis configuration to use with a Redis producer

            Args:
                address (str):
                    Redis address.
                    If not provided, the value of the REDIS_ADDR environment variable is used and defaults to "redis://localhost:6379".

                channel (str):
                    Redis channel to publish messages to.
                    If not provided, the value of the REDIS_CHANNEL environment variable is used and defaults to "scouter_monitoring".
            """
    ```

#### HTTPConfig
For those using HTTP, the `HTTPConfig` class allows you to specify the following parameters:

???note "Config Definition"
    ```python
    class HTTPConfig:
        server_uri: str
        username: str
        password: str
        auth_token: str

        def __init__(
            self,
            server_uri: Optional[str] = None,
            username: Optional[str] = None,
            password: Optional[str] = None,
            auth_token: Optional[str] = None,
        ) -> None:
            """HTTP configuration to use with the HTTPProducer.

            Args:
                server_uri:
                    URL of the HTTP server to publish messages to.
                    If not provided, the value of the HTTP_server_uri environment variable is used.

                username:
                    Username for basic authentication.

                password:
                    Password for basic authentication.

                auth_token:
                    Authorization token to use for authentication.

            """
    ```


### Inserting data

There are a variety of ways in which you can configure you api to send data. In this case, we're going to keep it simple and add the insertion logic in with the api prediction route.


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

    def to_features(self) -> Features: #(1)
        return Features(
            features=[
                Feature.float("feature_1", self.feature_1),
                Feature.float("feature_2", self.feature_2),
                Feature.float("feature_3", self.feature_3),
            ]
        )

app = FastAPI(lifespan=lifespan)

@app.post("/predict", response_model=Response)
async def predict(request: Request, payload: PredictRequest) -> Response:
    #... existing prediction logic
    request.app.state.queue["psi"].insert(payload.to_features()) #(2)
    return Response(value=prediction)
```

1. The queue expects either a `Features` object or a `Metrics` object (when inserting custom metrics). In this case we are manually implementing the features logic. However, you can also leverage the `FeatureMixin` class to convert a pydantic model into a `Features` object.
```python
from scouter import FeatureMixin
class PredictRequest(FeatureMixin, BaseModel):
    feature_1: float
    feature_2: float
    feature_3: float

request.to_features()
```
2. Access the queue from the app state using the assigned alias and insert

In the above logic, we access the queue via the request.app.state and the corresponding alias. We then call the insert method with the features we want to send to the Scouter server. This is a simple exchange of data, as the ScouterQueue will pass the features through a channel to a background worker that is running independently on a separate thread.

### Ready to go!

And that's all you need to get started for real-time model monitoring with Scouter. For more technical discussion on the ScouterQueue, please refer to the [ScouterQueue](../specs/ts-component-scouter-queue.md) documentation.