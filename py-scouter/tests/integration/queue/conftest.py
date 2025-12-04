from dataclasses import dataclass
from typing import Iterator
from unittest import mock

import pytest
from scouter.logging import LoggingConfig, LogLevel, RustyLogger
from scouter.mock import MockConfig

logger = RustyLogger.get_logger(
    LoggingConfig(log_level=LogLevel.Debug),
)


@dataclass
class MockEnvironment:
    mock_config: MockConfig


@pytest.fixture
def mock_environment() -> Iterator[MockEnvironment]:
    """
    Fixture that patches HttpConfig with MockConfig for testing.

    Yields:
        MockEnvironment: Contains the mock configuration.
    """
    with mock.patch("tests.integration.queue.test_mock.HttpConfig", MockConfig):
        yield MockEnvironment(mock_config=MockConfig())
