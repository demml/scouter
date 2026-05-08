from typing import Any

import pytest
from scouter.agent import MediaRef, Prompt, Provider, Score
from scouter.agent.openai import ChatMessage
from scouter.evaluate import (
    ComparisonOperator,
    EvalDataset,
    EvalRecord,
    ImageMedia,
    LLMJudgeTask,
)
from scouter.mock import LLMTestServer


def _score_task(prompt: Prompt) -> LLMJudgeTask:
    return LLMJudgeTask(
        id="chart_match",
        prompt=prompt,
        expected_value=0,
        context_path="score",
        operator=ComparisonOperator.GreaterThan,
        description="Judge chart match",
    )


def test_static_media_baked_into_prompt() -> None:
    """MediaRef.image_bytes baked into prompt dispatches through mocked OpenAI."""
    prompt = Prompt(
        messages=ChatMessage(
            role="user",
            content=[
                "Does this chart match the description: ${description}",
                "${media:chart}",
            ],
        ),
        provider=Provider.OpenAI,
        model="gpt-4o",
        output_type=Score,
    )
    prompt.bind_media_mut("chart", MediaRef.image_bytes("image/png", b"fakepng"))
    record = EvalRecord(
        context={"description": "Q3 revenue trend, monotonically increasing"},
    )

    with LLMTestServer():
        results = EvalDataset(records=[record], tasks=[_score_task(prompt)]).evaluate()

    assert results.successful_count == 1


def test_per_record_media_via_eval_record() -> None:
    """EvalRecord media binds to a ${media:chart} prompt placeholder."""
    prompt = Prompt(
        messages=ChatMessage(
            role="user",
            content=[
                "Does this chart match the description: ${description}",
                "${media:chart}",
            ],
        ),
        provider=Provider.OpenAI,
        model="gpt-4o",
        output_type=Score,
    )
    record = EvalRecord(
        context={"description": "Q3 revenue trend, monotonically increasing"},
        media=[
            ImageMedia(
                id="chart",
                url="https://example.com/run42/chart.png",
                mime_type="image/png",
            )
        ],
    )

    with LLMTestServer():
        results = EvalDataset(records=[record], tasks=[_score_task(prompt)]).evaluate()

    assert results.successful_count == 1


def test_image_media_rejects_multiple_sources() -> None:
    with pytest.raises(ValueError):
        ImageMedia(id="chart", url="https://x", bytes=b"...", mime_type="image/png")


def test_eval_record_rejects_unknown_media_type() -> None:
    media: list[Any] = [42]
    with pytest.raises(TypeError):
        EvalRecord(context={}, media=media)
