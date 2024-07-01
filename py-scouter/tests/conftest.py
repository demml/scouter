import pytest
import shutil
from typing import TypeVar, Generator
import numpy as np
from numpy.typing import NDArray
from scouter._scouter import DriftConfig, AlertRule, PercentageAlertRule
from unittest.mock import patch

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


@pytest.fixture
def mock_kafka_producer():
    with patch("confluent_kafka.Producer") as mocked_kafka:
        mocked_kafka.return_value.produce.return_value = None
        mocked_kafka.return_value.poll.return_value = 0
        mocked_kafka.return_value.flush.return_value = 0
        yield mocked_kafka
