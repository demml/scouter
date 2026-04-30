import os

import pytest
from scouter import GrpcConfig, ScouterEnv, ScouterQueue
from scouter.drift import AgentEvalProfile, AssertionTask, ComparisonOperator
from scouter.mock import MockConfig


@pytest.fixture(autouse=True)
def clean_offline_env():
    os.environ.pop("SCOUTER_OFFLINE", None)
    yield
    os.environ.pop("SCOUTER_OFFLINE", None)


def _minimal_profile() -> AgentEvalProfile:
    task = AssertionTask(
        id="check",
        context_path="response",
        operator=ComparisonOperator.NotEqual,
        expected_value="",
    )
    return AgentEvalProfile(tasks=[task], alias="test_offline")


def test_set_offline():
    assert not ScouterEnv.is_offline()
    ScouterEnv.set_offline()
    assert ScouterEnv.is_offline()


def test_is_offline_reads_env_directly():
    os.environ["SCOUTER_OFFLINE"] = "1"
    assert ScouterEnv.is_offline()


def test_queue_uses_mock_when_offline():
    ScouterEnv.set_offline()
    queue = ScouterQueue.from_profile(
        profile=_minimal_profile(),
        transport_config=GrpcConfig(),
        wait_for_startup=True,
    )
    assert isinstance(queue.transport_config, MockConfig)
    queue.shutdown()
