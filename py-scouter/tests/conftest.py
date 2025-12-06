import shutil
import uuid
from typing import Generator, TypeVar

import numpy as np
import pandas as pd
import pytest
from numpy.typing import NDArray
from scouter.alert import (
    CustomMetricAlertConfig,
    PsiAlertConfig,
    PsiChiSquareThreshold,
    PsiNormalThreshold,
    SlackDispatchConfig,
)
from scouter.drift import CustomMetricDriftConfig, PsiDriftConfig, SpcDriftConfig
from scouter.logging import LoggingConfig, LogLevel, RustyLogger

# Sets up logging for tests
RustyLogger.setup_logging(LoggingConfig(log_level=LogLevel.Info))

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
def feature_names() -> YieldFixture[list[str]]:
    features = ["feature_0", "feature_1", "feature_2"]
    yield features


@pytest.fixture(scope="function")
def cat_feature_names() -> YieldFixture[list[str]]:
    features = ["cat_feature_0", "cat_feature_1", "cat_feature_2"]
    yield features


@pytest.fixture(scope="function")
def drift_config() -> YieldFixture[SpcDriftConfig]:
    config = SpcDriftConfig(
        name=uuid.uuid4().hex,
        space=uuid.uuid4().hex,
    )
    yield config


@pytest.fixture(scope="function")
def psi_drift_config() -> YieldFixture[PsiDriftConfig]:
    config = PsiDriftConfig(
        name="test",
        space="test",
    )
    yield config


@pytest.fixture(scope="function")
def psi_drift_normal_threshold_psi_config() -> YieldFixture[PsiDriftConfig]:
    config = PsiDriftConfig(
        name="test",
        space="test",
        alert_config=PsiAlertConfig(
            threshold=PsiNormalThreshold(),
        ),
    )
    yield config


@pytest.fixture(scope="function")
def psi_drift_chi_threshold_psi_config() -> YieldFixture[PsiDriftConfig]:
    config = PsiDriftConfig(
        name="test",
        space="test",
        alert_config=PsiAlertConfig(
            threshold=PsiChiSquareThreshold(),
        ),
    )
    yield config


@pytest.fixture(scope="function")
def psi_drift_config_with_categorical_features(
    cat_feature_names: list[str],
) -> YieldFixture[PsiDriftConfig]:
    config = PsiDriftConfig(
        name="test",
        space="test",
        categorical_features=cat_feature_names,
    )
    yield config


@pytest.fixture(scope="function")
def pandas_dataframe(array: NDArray, feature_names: list[str]) -> YieldFixture:
    df = pd.DataFrame(array, columns=feature_names)

    yield df

    cleanup()


@pytest.fixture(scope="function")
def polars_dataframe(array: NDArray, feature_names: list[str]) -> YieldFixture:
    import polars as pl

    df = pl.from_numpy(array, schema=feature_names)

    yield df

    cleanup()


@pytest.fixture(scope="function")
def categorical_polars_dataframe(cat_feature_names: list[str], nrow: int) -> YieldFixture:
    import polars as pl

    ints = np.random.randint(1, 4, nrow).reshape(-1, 1)

    ints2 = np.random.randint(5, 10, nrow).reshape(-1, 1)

    ints3 = np.random.randint(10, 20, nrow).reshape(-1, 1)

    array = np.concatenate([ints, ints2, ints3], axis=1)

    df = pl.from_numpy(array, schema=cat_feature_names)

    df = df.with_columns([pl.col(column_name).cast(str).cast(pl.Categorical) for column_name in cat_feature_names])

    yield df

    cleanup()


@pytest.fixture(scope="function")
def polars_dataframe_multi_dtype(polars_dataframe, categorical_polars_dataframe) -> YieldFixture:
    import polars as pl

    df = pl.concat([polars_dataframe, categorical_polars_dataframe], how="horizontal")

    yield df

    cleanup()


@pytest.fixture(scope="function")
def polars_dataframe_multi_dtype_drift(polars_dataframe_multi_dtype, cat_feature_names) -> YieldFixture:
    import polars as pl

    # Create a copy and modify the first categorical column to simulate drift
    df = polars_dataframe_multi_dtype.clone()

    # Scale the first categorical column
    first_cat_col = cat_feature_names[0]
    df = df.with_columns((pl.col(first_cat_col).cast(pl.Int32) + 3).cast(str).cast(pl.Categorical).alias(first_cat_col))

    yield df
    cleanup()


@pytest.fixture(scope="function")
def pandas_categorical_dataframe(cat_feature_names: list[str]) -> YieldFixture:
    data = {}
    for i, col_name in enumerate(cat_feature_names):
        start_letter = ord("a") + (i * 6)
        categories = [chr(start_letter + j) for j in range(6)]
        data[col_name] = pd.Categorical(categories * 333)

    df = pd.DataFrame(data)
    yield df

    cleanup()


@pytest.fixture(scope="function")
def custom_metric_drift_config() -> YieldFixture[CustomMetricDriftConfig]:
    config = CustomMetricDriftConfig(
        name="test",
        space="test",
        alert_config=CustomMetricAlertConfig(
            schedule="0 0 * * * *",
            dispatch_config=SlackDispatchConfig(channel="test_channel"),
        ),
    )
    yield config
