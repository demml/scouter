from pathlib import Path
from tempfile import TemporaryDirectory

import pytest
from pydantic import BaseModel
from scouter.alert import AlertThreshold
from scouter.drift import Drifter, LLMDriftConfig, LLMDriftProfile, LLMMetric
from scouter.llm import Agent, Prompt, Score, Task, Workflow
from scouter.mock import LLMTestServer
from scouter.queue import LLMRecord


class TaskOutput(BaseModel):
    task_output: str


def test_llm_drift_profile_from_metrics():
    with LLMTestServer():
        prompt = Prompt(
            message="${input} + ${response}?",
            system_instruction="You are a helpful assistant.",
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

        _profile = LLMDriftProfile(
            config=LLMDriftConfig(),
            metrics=[metric1, metric2],
        )


def test_llm_drift_profile_from_workflow():
    with LLMTestServer():
        start_prompt = Prompt(
            message="${input} + ${response}?",
            system_instruction="You are a helpful assistant.",
            model="gpt-4o",
            provider="openai",
        )

        end_prompt = Prompt(
            message="Foo bar",
            system_instruction="You are a helpful assistant.",
            model="gpt-4o",
            provider="openai",
            response_format=Score,
        )

        open_agent = Agent("openai")
        workflow = Workflow(name="test_workflow")
        workflow.add_agent(open_agent)
        workflow.add_tasks(
            [  # allow adding list of tasks
                Task(
                    prompt=start_prompt,
                    agent_id=open_agent.id,
                    id="start_task",
                ),
                Task(
                    prompt=end_prompt,
                    agent_id=open_agent.id,
                    id="relevance",
                    dependencies=["start_task"],
                ),
            ]
        )

        metric = LLMMetric(
            name="relevance",
            value=5.0,
            alert_threshold=AlertThreshold.Below,
        )

        profile = LLMDriftProfile(
            config=LLMDriftConfig(),
            workflow=workflow,
            metrics=[metric],
        )

        assert profile.config is not None

        assert isinstance(profile.model_dump_json(), str)
        assert isinstance(profile.model_dump(), dict)

        with TemporaryDirectory() as temp_dir:
            path = Path(temp_dir) / "profile.json"
            profile.save_to_json(path)
            assert (Path(temp_dir) / "profile.json").exists()

            with open(path, "r") as f:
                LLMDriftProfile.model_validate_json(f.read())


def test_llm_drift_profile_from_metrics_fail():
    with LLMTestServer():
        prompt = Prompt(
            message="foo bar",
            system_instruction="You are a helpful assistant.",
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
            value=10.0,
            alert_threshold=AlertThreshold.Above,
        )

        # Drift profile with no required parameters should raise an error
        with pytest.raises(RuntimeError, match="LLM Metric requires at least one bound parameter"):
            _profile = LLMDriftProfile(
                config=LLMDriftConfig(),
                metrics=[metric1],
            )

        # Drift profile with metric without prompt should raise an error
        with pytest.raises(RuntimeError, match="Missing prompt in LLM Metric"):
            _profile = LLMDriftProfile(
                config=LLMDriftConfig(),
                metrics=[metric2],
            )


def test_llm_drift_profile_from_workflow_fail():
    with LLMTestServer():
        start_prompt = Prompt(
            message="Foo bar",
            system_instruction="You are a helpful assistant.",
            model="gpt-4o",
            provider="openai",
        )

        end_prompt = Prompt(
            message="Foo bar",
            system_instruction="You are a helpful assistant.",
            model="gpt-4o",
            provider="openai",
            response_format=Score,
        )

        open_agent = Agent("openai")
        workflow = Workflow(name="test_workflow")
        workflow.add_agent(open_agent)
        workflow.add_tasks(
            [  # allow adding list of tasks
                Task(
                    prompt=start_prompt,
                    agent_id=open_agent.id,
                    id="start_task",
                ),
                Task(
                    prompt=end_prompt,
                    agent_id=open_agent.id,
                    id="relevance",
                    dependencies=["start_task"],
                ),
            ]
        )

        metric = LLMMetric(
            name="relevance",
            value=5.0,
            alert_threshold=AlertThreshold.Below,
        )

        with pytest.raises(RuntimeError, match="LLM Metric requires at least one bound parameter"):
            _profile = LLMDriftProfile(
                config=LLMDriftConfig(),
                workflow=workflow,
                metrics=[metric],
            )


def test_llm_drift_profile_workflow_run_context():
    with LLMTestServer():
        # this should bind the input and response context and return TaskOutput
        start_prompt = Prompt(
            message="${input} + ${response}?",
            system_instruction="You are a helpful assistant.",
            model="gpt-4o",
            provider="openai",
            response_format=TaskOutput,
        )

        # this should bind the task_output context and return Score
        end_prompt = Prompt(
            message="${task_output}",
            system_instruction="You are a helpful assistant.",
            model="gpt-4o",
            provider="openai",
            response_format=Score,
        )

        open_agent = Agent("openai")
        workflow = Workflow(name="test_workflow")
        workflow.add_agent(open_agent)
        workflow.add_tasks(
            [  # allow adding list of tasks
                Task(
                    prompt=start_prompt,
                    agent_id=open_agent.id,
                    id="start_task",
                ),
                Task(
                    prompt=end_prompt,
                    agent_id=open_agent.id,
                    id="relevance",
                    dependencies=["start_task"],
                ),
            ]
        )

        global_context = {
            "input": "What is the capital of France?",
            "response": "The capital of France is Paris.",
        }
        result = workflow.run(
            global_context=global_context,
        )

        assert (
            result.tasks.get("start_task").prompt.message[0].unwrap()
            == '"What is the capital of France?" + "The capital of France is Paris."?'
        )

        assert result.tasks.get("relevance").prompt.message[1].unwrap() == '"foo bar"'


def test_llm_drifter():
    with LLMTestServer():
        # this should bind the input and response context and return TaskOutput
        eval_prompt = Prompt(
            message="${input} + ${response}?",
            system_instruction="You are a helpful assistant. Output a score between 1 and 5",
            model="gpt-4o",
            provider="openai",
            response_format=Score,
        )

        profile = LLMDriftProfile(
            config=LLMDriftConfig(),
            metrics=[
                LLMMetric(
                    name="relevance",
                    prompt=eval_prompt,
                    value=5.0,
                    alert_threshold=AlertThreshold.Below,
                )
            ],
        )

        record = LLMRecord(
            context={
                "input": "What is the capital of France?",
                "response": "The capital of France is Paris.",
            },
        )

        drifter = Drifter()
        results = drifter.compute_drift(record, profile)

        assert len(results.records) == 1
        assert results.records[0].metric == "relevance"
        assert results.records[0].value == 5.0
