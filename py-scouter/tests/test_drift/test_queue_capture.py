"""Unit tests for ScouterQueue offline record capture (enable_capture / disable_capture / drain)."""

import pytest
from scouter.agent import Prompt, Score
from scouter.drift import AgentEvalProfile, ComparisonOperator, LLMJudgeTask
from scouter.mock import LLMTestServer, MockConfig
from scouter.queue import EvalRecord, ScouterQueue


def _minimal_profile() -> AgentEvalProfile:
    prompt = Prompt(
        messages="${input} + ${response}?",
        system_instructions="You are a helpful assistant.",
        model="gpt-4o",
        provider="openai",
        output_type=Score,
    )
    task = LLMJudgeTask(
        id="query_relevance",
        context_path="score",
        prompt=prompt,
        operator=ComparisonOperator.GreaterThanOrEqual,
        expected_value=3,
    )
    return AgentEvalProfile(tasks=[task], alias="test")


@pytest.fixture
def genai_queue() -> ScouterQueue:
    with LLMTestServer():
        profile = _minimal_profile()
        return ScouterQueue.from_profile(
            profile=profile,
            transport_config=MockConfig(),
            wait_for_startup=True,
        )


@pytest.fixture
def eval_record() -> EvalRecord:
    return EvalRecord(context={"input": "What is 2+2?", "response": "4"})


def test_capture_disabled_by_default(genai_queue: ScouterQueue) -> None:
    """drain_all_records returns an empty dict when capture has never been enabled."""
    assert genai_queue.drain_all_records() == {}


def test_enable_capture_and_drain(genai_queue: ScouterQueue, eval_record: EvalRecord) -> None:
    """Records inserted after enable_capture are returned by drain_records."""
    genai_queue.enable_capture()
    genai_queue["test"].insert(eval_record)
    records = genai_queue.drain_records("test")
    assert len(records) == 1
    assert isinstance(records[0], EvalRecord)


def test_drain_clears_buffer(genai_queue: ScouterQueue, eval_record: EvalRecord) -> None:
    """drain_records empties the buffer so a second call returns nothing."""
    genai_queue.enable_capture()
    genai_queue["test"].insert(eval_record)
    genai_queue.drain_records("test")
    assert genai_queue.drain_records("test") == []


def test_disable_capture_frees_buffer(genai_queue: ScouterQueue, eval_record: EvalRecord) -> None:
    """disable_capture discards buffered records; drain_all_records returns empty afterwards."""
    genai_queue.enable_capture()
    genai_queue["test"].insert(eval_record)
    genai_queue.disable_capture()
    assert genai_queue.drain_all_records() == {}
