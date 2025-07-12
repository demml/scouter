from scouter.alert import AlertThreshold, LLMAlertConfig, LLMMetricAlertCondition
from scouter.drift import LLMDriftConfig, LLMDriftProfile, LLMMetric
from scouter.mock import Agent, Prompt, Score, Task, Workflow
from scouter.mock import OpenAITestServer


def test_llm_drift_profile():
    with OpenAITestServer():
        prompt = Prompt(
            user_message="${input} + ${response}?",
            system_message="You are a helpful assistant.",
            model="gpt-4o",
            provider="openai",
            response_format=Score,
        )

        metric1 = LLMMetric(
            name="test_metric",
            prompt=prompt,
            value=5.0,
            alert_threshold=AlertThreshold.Below,
        )
        metric2 = LLMMetric(
            name="test_metric_2",
            prompt=prompt,
            value=10.0,
            alert_threshold=AlertThreshold.Above,
        )

        profile = LLMDriftProfile(
            config=LLMDriftConfig(),
            metrics=[metric1, metric2],
        )
