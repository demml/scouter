from typing import cast
from pydantic import BaseModel
from scouter._scouter import GenAIEvalResultSet
from scouter.alert import AlertThreshold, AlertCondition
from scouter.drift import (
    Drifter,
    GenAIDriftConfig,
    GenAIEvalProfile,
    GenAIAlertConfig,
    LLMJudgeTask,
    ComparisonOperator,
)
from scouter.genai import Prompt, Score
from scouter.mock import LLMTestServer
from scouter.queue import GenAIEvalRecord


class TaskOutput(BaseModel):
    task_output: str


def test_genai_drift_profile_from_task():
    with LLMTestServer():
        prompt = Prompt(
            messages="${input} + ${response}?",
            system_instructions="You are a helpful assistant.",
            model="gpt-4o",
            provider="openai",
            output_type=Score,
        )

        task = LLMJudgeTask(
            id="query_relevance",
            field_path="score",
            prompt=prompt,
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=3,
        )

        _profile = GenAIEvalProfile(config=GenAIDriftConfig(), tasks=[task])


def test_genai_drifter():
    with LLMTestServer():
        # this should bind the input and response context and return TaskOutput
        eval_prompt = Prompt(
            messages="${input} + ${response}?",
            system_instructions="You are a helpful assistant. Output a score between 1 and 5",
            model="gpt-4o",
            provider="openai",
            output_type=Score,
        )

        task = LLMJudgeTask(
            id="query_relevance",
            field_path="score",
            prompt=eval_prompt,
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=3,
        )

        profile = GenAIEvalProfile(
            config=GenAIDriftConfig(
                alert_config=GenAIAlertConfig(
                    alert_condition=AlertCondition(
                        baseline_value=0.80,
                        alert_threshold=AlertThreshold.Below,
                        delta=0.10,
                    )
                )
            ),
            tasks=[task],
        )

        record = GenAIEvalRecord(
            context={
                "input": "What is the capital of France?",
                "response": "The capital of France is Paris.",
            },
        )

        drifter = Drifter()
        results = cast(GenAIEvalResultSet, drifter.compute_drift([record], profile))

        assert len(results.records) == 1
