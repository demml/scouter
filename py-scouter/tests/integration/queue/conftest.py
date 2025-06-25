from dataclasses import dataclass
from typing import Iterator
from unittest import mock

import pytest
from scouter.test import MockConfig


@dataclass
class MockEnvironment:
    mock_config: MockConfig


@pytest.fixture
def mock_environment() -> Iterator[MockEnvironment]:
    """
    Fixture that patches HTTPConfig with MockConfig for testing.

    Yields:
        MockEnvironment: Contains the mock configuration.
    """
    with mock.patch("tests.integration.queue.test_mock.HTTPConfig", MockConfig):
        yield MockEnvironment(mock_config=MockConfig())
