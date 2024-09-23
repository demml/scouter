import pytest
import shutil
from typing import TypeVar, Generator
import numpy as np
from numpy.typing import NDArray
from scouter._scouter import DriftConfig, AlertRule, PercentageAlertRule, AlertConfig
from unittest.mock import patch
from httpx import Response
from fastapi import FastAPI, Request
from scouter.integrations.fastapi import ScouterRouter
from scouter import Drifter, DriftProfile, KafkaConfig
from fastapi.testclient import TestClient
from pydantic import BaseModel

T = TypeVar("T")
YieldFixture = Generator[T, None, None]


def cleanup() -> None:
    """Removes temp files"""

    # delete lightning_logs
    shutil.rmtree("assets", ignore_errors=True)


@pytest.fixture(scope="function")
def array() -> YieldFixture[NDArray]:
    array = np.random.rand(1000, 3)
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
def drift_config() -> YieldFixture[DriftConfig]:
    config = DriftConfig(name="test", repository="test")
    yield config


@pytest.fixture(scope="function")
def drift_config_percentage() -> YieldFixture[DriftConfig]:
    alert_config = AlertConfig(
        alert_rule=AlertRule(
            percentage_rule=PercentageAlertRule(0.1),
        )
    )
    config = DriftConfig(
        name="test",
        repository="test",
        alert_config=alert_config,
    )

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
def mock_httpx_producer():
    with patch("httpx.Client") as mocked_client:
        mocked_client.return_value.post.return_value = Response(
            status_code=200,
            json={"access_token": "test-token"},
        )

        yield mocked_client


@pytest.fixture
def drift_profile(array: NDArray) -> DriftProfile:
    drifter = Drifter()
    profile: DriftProfile = drifter.create_drift_profile(array)

    return profile


class TestResponse(BaseModel):
    message: str


@pytest.fixture
def client(mock_kafka_producer, drift_profile: DriftProfile) -> TestClient:
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
