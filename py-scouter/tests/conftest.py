import shutil
from typing import Generator, TypeVar

import numpy as np
import pandas as pd
import pytest
from numpy.typing import NDArray
from scouter.alert import (
    AlertDispatchType,
    CustomMetricAlertConfig,
    PsiAlertConfig,
    SpcAlertConfig,
)
from scouter.drift import CustomMetricDriftConfig, PsiDriftConfig, SpcDriftConfig
from scouter.logging import LoggingConfig, LogLevel, RustyLogger

# Sets up logging for tests
RustyLogger.setup_logging(LoggingConfig(log_level=LogLevel.Debug))

T = TypeVar("T")
YieldFixture = Generator[T, None, None]


def cleanup() -> None:
    """Removes temp files"""

    # delete lightning_logs
    shutil.rmtree("assets", ignore_errors=True)


@pytest.fixture(scope="function")
def nrow() -> int:
    return 1000


@pytest.fixture(scope="function")
def array(nrow: int) -> YieldFixture[NDArray]:
    array = np.random.rand(nrow, 3)
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
    config = SpcDriftConfig(
        name="test",
        repository="test",
        alert_config=SpcAlertConfig(
            features_to_monitor=["column_0", "column_1", "column_2"],
        ),
    )
    yield config


@pytest.fixture(scope="function")
def psi_drift_config() -> YieldFixture[PsiDriftConfig]:
    config = PsiDriftConfig(
        name="test",
        repository="test",
        alert_config=PsiAlertConfig(
            features_to_monitor=["column_0", "column_1", "column_2"],
        ),
    )
    yield config


@pytest.fixture(scope="function")
def pandas_dataframe(array: NDArray) -> YieldFixture:

    df = pd.DataFrame(array)

    # change column names
    df.rename(columns={0: "column_0", 1: "column_1", 2: "column_2"}, inplace=True)

    yield df

    cleanup()


@pytest.fixture(scope="function")
def pandas_dataframe_multi_type(array: NDArray) -> YieldFixture:

    df = pd.DataFrame(array)

    # change column names
    df.rename(columns={0: "column_0", 1: "column_1", 2: "column_2"}, inplace=True)

    # change column_0 to be int
    df["column_0"] = df["column_0"].astype(int)

    # column 3 should be string of ints between 1 and 3
    # df["column_3"]  = np.random.randint(1, 4, df.shape[0])
    # df["column_3"] = df["column_3"].astype(str)

    yield df

    cleanup()


@pytest.fixture(scope="function")
def polars_dataframe(array: NDArray) -> YieldFixture:
    import polars as pl

    df = pl.from_numpy(array, schema=["column_0", "column_1", "column_2"])

    yield df

    cleanup()


@pytest.fixture(scope="function")
def polars_dataframe_multi_dtype(array: NDArray, nrow: int) -> YieldFixture:
    import polars as pl

    # add column of ints between 1 and 3
    ints = np.random.randint(1, 4, nrow).reshape(-1, 1)

    ints2 = np.random.randint(5, 10, nrow).reshape(-1, 1)

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

    df = pd.DataFrame(
        {
            "cat1": pd.Categorical(["a", "b", "c", "e", "f", "g"] * 333),
            "cat2": pd.Categorical(["h", "i", "j", "k", "l", "m"] * 333),
            "cat3": pd.Categorical(["n", "o", "p", "q", "r", "s"] * 333),
        }
    )

    yield df

    cleanup()


@pytest.fixture(scope="function")
def custom_metric_drift_config() -> YieldFixture[CustomMetricDriftConfig]:
    config = CustomMetricDriftConfig(
        name="test",
        repository="test",
        alert_config=CustomMetricAlertConfig(dispatch_type=AlertDispatchType.Slack, schedule="0 0 * * * *"),
    )
    yield config
