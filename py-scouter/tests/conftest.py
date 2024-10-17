import pytest
import shutil
from typing import TypeVar, Generator
import numpy as np
import pandas as pd
from numpy.typing import NDArray
from scouter._scouter import SpcDriftConfig
from unittest.mock import patch
from httpx import Response
from fastapi import FastAPI, Request
from scouter.integrations.fastapi import ScouterRouter, FastAPIScouterObserver
from scouter import Drifter, SpcDriftProfile, KafkaConfig, HTTPConfig, DriftType
from fastapi.testclient import TestClient
from pydantic import BaseModel
from scouter.observability.observer import ScouterObserver

T = TypeVar("T")
YieldFixture = Generator[T, None, None]


def generate_data() -> pd.DataFrame:
    """Create a fake data frame for testing"""
    n = 10_00

    X_train = np.random.normal(-4, 2.0, size=(n, 10))

    col_names = []
    for i in range(0, X_train.shape[1]):
        col_names.append(f"col_{i}")

    X = pd.DataFrame(X_train, columns=col_names)
    X["col_11"] = np.random.randint(1, 20, size=(n, 1))
    X["target"] = np.random.randint(1, 10, size=(n, 1))

    return X


class TestResponse(BaseModel):
    message: str


class TestDriftRecord(BaseModel):
    name: str
    repository: str
    version: str
    feature: str
    value: float


class PredictRequest(BaseModel):
    feature_0: float
    feature_1: float
    feature_2: float


def cleanup() -> None:
    """Removes temp files"""

    # delete lightning_logs
    shutil.rmtree("assets", ignore_errors=True)


@pytest.fixture(scope="function")
def array() -> YieldFixture[NDArray]:
    array = np.random.rand(10000, 30)
    # add 1 to first column
    array[:, 0] += 1
    # add 2 to second column
    array[:, 1] += 2
    # add 3 to third column
    array[:, 2] += 3

    yield array

    cleanup()


@pytest.fixture(scope="function")
def multivariate_array() -> YieldFixture[NDArray]:
    xx = np.array([-0.51, 51.2])
    yy = np.array([0.33, 51.6])
    means = [xx.mean(), yy.mean()]
    stds = [xx.std() / 3, yy.std() / 3]
    corr = 0.8  # correlation
    covs = [
        [stds[0] ** 2, stds[0] * stds[1] * corr],
        [stds[0] * stds[1] * corr, stds[1] ** 2],
    ]

    yield np.random.multivariate_normal(means, covs, 1000)

    cleanup()


@pytest.fixture(scope="function")
def multivariate_array_drift() -> YieldFixture[NDArray]:
    xx = np.array([-0.21, 21.2])
    yy = np.array([0.13, 31.6])
    means = [xx.mean(), yy.mean()]
    stds = [xx.std() / 3, yy.std() / 3]
    corr = 0.8  # correlation
    covs = [
        [stds[0] ** 2, stds[0] * stds[1] * corr],
        [stds[0] * stds[1] * corr, stds[1] ** 2],
    ]

    yield np.random.multivariate_normal(means, covs, 1000)

    cleanup()


@pytest.fixture(scope="function")
def drift_config() -> YieldFixture[SpcDriftConfig]:
    config = SpcDriftConfig(name="test", repository="test")
    yield config


@pytest.fixture(scope="function")
def pandas_dataframe(array: NDArray) -> YieldFixture:
    import pandas as pd

    df = pd.DataFrame(array)

    # change column names
    df.rename(columns={0: "column_0", 1: "column_1", 2: "column_2"}, inplace=True)

    yield df

    cleanup()


@pytest.fixture(scope="function")
def polars_dataframe(array: NDArray) -> YieldFixture:
    import polars as pl

    df = pl.from_numpy(array, schema=["column_0", "column_1", "column_2"])

    yield df

    cleanup()


@pytest.fixture(scope="function")
def polars_dataframe_multi_dtype(array: NDArray) -> YieldFixture:
    import polars as pl

    # add column of ints between 1 and 3
    ints = np.random.randint(1, 4, 1000).reshape(-1, 1)

    ints2 = np.random.randint(5, 10, 1000).reshape(-1, 1)

    # add to array
    array = np.concatenate([ints2, array, ints], axis=1)

    df = pl.from_numpy(array, schema=["cat1", "num1", "num2", "num3", "cat2"])

    # add categorical column
    df = df.with_columns(
        [
            pl.col("cat1").cast(str).cast(pl.String),
            pl.col("cat2").cast(str).cast(pl.Categorical),
        ]
    )

    yield df

    cleanup()


@pytest.fixture(scope="function")
def polars_dataframe_multi_dtype_drift(array: NDArray) -> YieldFixture:
    import polars as pl

    # add column of ints between 1 and 3
    ints = np.random.randint(4, 6, 1000).reshape(-1, 1)

    ints2 = np.random.randint(5, 10, 1000).reshape(-1, 1)

    # add to array
    array = np.concatenate([ints2, array, ints], axis=1)

    df = pl.from_numpy(array, schema=["cat1", "num1", "num2", "num3", "cat2"])

    # add categorical column
    df = df.with_columns(
        [
            pl.col("cat1").cast(str).cast(pl.String),
            pl.col("cat2").cast(str).cast(pl.Categorical),
        ]
    )

    yield df

    cleanup()


@pytest.fixture(scope="function")
def pandas_categorical_dataframe() -> YieldFixture:
    import pandas as pd

    df = pd.DataFrame(
        {
            "cat1": pd.Categorical(["a", "b", "c", "e", "f", "g"] * 333),
            "cat2": pd.Categorical(["h", "i", "j", "k", "l", "m"] * 333),
            "cat3": pd.Categorical(["n", "o", "p", "q", "r", "s"] * 333),
        }
    )

    yield df

    cleanup()


@pytest.fixture
def mock_kafka_producer():
    with patch("confluent_kafka.Producer") as mocked_kafka:
        mocked_kafka.return_value.produce.return_value = None
        mocked_kafka.return_value.poll.return_value = 0
        mocked_kafka.return_value.flush.return_value = 0
        yield mocked_kafka


@pytest.fixture
def mock_rabbit_connection():
    class BlockingConnection:
        def __init__(self, *args, **kwargs):
            pass

        def channel(self):
            return self

        def exchange_declare(self, *args, **kwargs):
            pass

        def queue_declare(self, *args, **kwargs):
            pass

        def queue_bind(self, *args, **kwargs):
            pass

        def basic_publish(self, *args, **kwargs):
            pass

        def close(self):
            pass

    with patch("pika.BlockingConnection") as mocked_connection:
        mocked_connection.return_value = BlockingConnection()
        yield mocked_connection


@pytest.fixture
def mock_httpx_producer():
    with patch("httpx.Client") as mocked_client:
        mocked_client.return_value.post.return_value = Response(
            status_code=200,
            json={"access_token": "test-token"},
        )

        yield mocked_client


@pytest.fixture
def client(mock_kafka_producer, drift_profile: SpcDriftProfile) -> TestClient:
    config = KafkaConfig(
        topic="test-topic",
        brokers="localhost:9092",
        raise_on_err=True,
        config={"bootstrap.servers": "localhost:9092"},
    )

    app = FastAPI()
    router = ScouterRouter(drift_profile=drift_profile, config=config)

    # create a test route
    @router.get("/test", response_model=TestResponse)
    async def test_route(request: Request) -> TestResponse:
        request.state.scouter_data = {"test": "data"}
        return TestResponse(message="success")

    app.include_router(router)
    return TestClient(app)


@pytest.fixture
def drift_profile():
    data = generate_data()

    # create drift config (usually associated with a model name, repository name, version)
    config = SpcDriftConfig(repository="scouter", name="model", version="0.1.0")

    # create drifter
    drifter = Drifter(DriftType.SPC)

    # create drift profile
    profile = drifter.create_drift_profile(data, config)

    return profile


@pytest.fixture
def client_insert(
    drift_profile: SpcDriftProfile,
) -> TestClient:
    config = HTTPConfig(server_url="http://testserver")
    app = FastAPI()

    # define scouter router
    scouter_router = ScouterRouter(drift_profile=drift_profile, config=config)
    observer = FastAPIScouterObserver(drift_profile, config)

    @scouter_router.post("/predict", response_model=TestResponse)
    async def predict(request: Request, payload: PredictRequest) -> TestResponse:
        request.state.scouter_data = payload.model_dump()

        return TestResponse(message="success")

    app.include_router(scouter_router)
    observer.observe(app)

    return TestClient(app)


@pytest.fixture
def kafka_config():
    return KafkaConfig(
        topic="test-topic",
        brokers="localhost:9092",
        raise_on_err=True,
    )


@pytest.fixture
def scouter_observer(kafka_config: KafkaConfig):
    with patch("scouter.Observer") as MockObserver:
        mock_observer = MockObserver.return_value
        scouter_observer = ScouterObserver(
            "repo",
            "name",
            "version",
            kafka_config,
        )
        yield scouter_observer, mock_observer
        scouter_observer.stop()
