import pytest
import shutil
from typing import TypeVar, Generator
import numpy as np
from numpy.typing import NDArray
from scouter._scouter import DriftConfig, AlertRule, PercentageAlertRule
from unittest.mock import patch
from httpx import Response

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
def monitor_config() -> YieldFixture[DriftConfig]:
    config = DriftConfig(name="test", repository="test")
    yield config


@pytest.fixture(scope="function")
def monitor_config_percentage() -> YieldFixture[DriftConfig]:
    config = DriftConfig(
        name="test",
        repository="test",
        alert_rule=AlertRule(percentage_rule=PercentageAlertRule(0.1)),
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
