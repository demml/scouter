import pytest
from scouter.evaluate import (
    AssertionTask,
    ComparisonOperator,
    GenAIEvalRecord,
    LLMJudgeTask,
)
from scouter.genai import Prompt, Provider, Role, Score
from scouter.genai.openai import ChatMessage


@pytest.fixture
def assertion_task_foo() -> AssertionTask:
    return AssertionTask(
        id="input_foo_check",
        field_path="input.foo",
        operator=ComparisonOperator.Equals,
        expected_value="bar",
        description="Check that input.foo equals 'bar'",
    )


@pytest.fixture
def assertion_task_bar() -> AssertionTask:
    return AssertionTask(
        id="input_bar_check",
        field_path="input.bar",
        operator=ComparisonOperator.IsNumeric,
        expected_value=True,
        description="Check that input.bar is numeric",
    )


@pytest.fixture
def assertion_task_baz() -> AssertionTask:
    return AssertionTask(
        id="input_baz_check",
        field_path="input.baz",
        operator=ComparisonOperator.HasLengthEqual,
        expected_value=3,
        description="Check that input.baz has length equal to 3",
    )


@pytest.fixture
def llm_judge_query_relevance() -> LLMJudgeTask:
    prompt = Prompt(
        messages=ChatMessage(
            role=Role.User.as_str(),
            content="What is the score ${input}",
        ),
        system_instructions=ChatMessage(
            role=Role.Developer.as_str(),
            content="You are a helpful assistant.",
        ),
        model="gpt-4o",
        provider=Provider.OpenAI,
        output_type=Score,
    )
    return LLMJudgeTask(
        id="query_relevance",
        field_path="score",
        prompt=prompt,
        operator=ComparisonOperator.GreaterThanOrEqual,
        expected_value=3,
        description="Check that input.baz has length equal to 3",
    )


@pytest.fixture
def query_relevance_score_assertion_task() -> AssertionTask:
    return AssertionTask(
        id="assert_score",
        field_path="query_relevance.score",
        operator=ComparisonOperator.IsNumeric,
        expected_value=True,
        description="Check that score is numeric",
        depends_on=["query_relevance"],
    )


@pytest.fixture
def query_relevance_reason_assertion_task() -> AssertionTask:
    return AssertionTask(
        id="assert_reason",
        field_path="query_relevance.reason",
        operator=ComparisonOperator.IsString,
        expected_value=True,
        description="Check that reason is alphabetic",
        depends_on=["query_relevance"],
    )


@pytest.fixture
def base_assertion_tasks():
    """Shared assertion tasks for comparison tests."""
    return [
        AssertionTask(
            id="quality_check",
            field_path="metrics.quality_score",
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=7,
            description="Quality score must be at least 7/10",
        ),
        AssertionTask(
            id="accuracy_check",
            field_path="metrics.accuracy_score",
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=8,
            description="Accuracy score must be at least 8/10",
        ),
        AssertionTask(
            id="provides_solution",
            field_path="response.provides_solution",
            operator=ComparisonOperator.Equals,
            expected_value=True,
            description="Response must provide a solution",
        ),
        AssertionTask(
            id="acknowledges_concern",
            field_path="response.acknowledges_concern",
            operator=ComparisonOperator.Equals,
            expected_value=True,
            description="Response must acknowledge concern",
        ),
    ]


@pytest.fixture
def baseline_records():
    """Baseline records with mixed success rates."""
    return [
        GenAIEvalRecord(
            context={
                "metrics": {"quality_score": 5, "accuracy_score": 6},
                "response": {"provides_solution": False, "acknowledges_concern": False},
            },
            id="record_1",
        ),
        GenAIEvalRecord(
            context={
                "metrics": {"quality_score": 7, "accuracy_score": 8},
                "response": {"provides_solution": True, "acknowledges_concern": True},
            },
            id="record_2",
        ),
        GenAIEvalRecord(
            context={
                "metrics": {"quality_score": 6, "accuracy_score": 7},
                "response": {"provides_solution": True, "acknowledges_concern": False},
            },
            id="record_3",
        ),
        GenAIEvalRecord(
            context={
                "metrics": {"quality_score": 8, "accuracy_score": 9},
                "response": {"provides_solution": True, "acknowledges_concern": True},
            },
            id="record_4",
        ),
        GenAIEvalRecord(
            context={
                "metrics": {"quality_score": 5, "accuracy_score": 6},
                "response": {"provides_solution": False, "acknowledges_concern": True},
            },
            id="record_5",
        ),
    ]


@pytest.fixture
def improved_records():
    """Improved records with better scores across the board."""
    return [
        GenAIEvalRecord(
            context={
                "metrics": {"quality_score": 8, "accuracy_score": 9},
                "response": {"provides_solution": True, "acknowledges_concern": True},
            },
            id="record_1",
        ),
        GenAIEvalRecord(
            context={
                "metrics": {"quality_score": 9, "accuracy_score": 10},
                "response": {"provides_solution": True, "acknowledges_concern": True},
            },
            id="record_2",
        ),
        GenAIEvalRecord(
            context={
                "metrics": {"quality_score": 7, "accuracy_score": 8},
                "response": {"provides_solution": True, "acknowledges_concern": True},
            },
            id="record_3",
        ),
        GenAIEvalRecord(
            context={
                "metrics": {"quality_score": 9, "accuracy_score": 10},
                "response": {"provides_solution": True, "acknowledges_concern": True},
            },
            id="record_4",
        ),
        GenAIEvalRecord(
            context={
                "metrics": {"quality_score": 8, "accuracy_score": 9},
                "response": {"provides_solution": True, "acknowledges_concern": True},
            },
            id="record_5",
        ),
    ]


@pytest.fixture
def regressed_records():
    """Records showing regression in some metrics."""
    return [
        GenAIEvalRecord(
            context={
                "metrics": {"quality_score": 4, "accuracy_score": 5},
                "response": {"provides_solution": False, "acknowledges_concern": False},
            },
            id="record_1",
        ),
        GenAIEvalRecord(
            context={
                "metrics": {"quality_score": 6, "accuracy_score": 7},
                "response": {"provides_solution": True, "acknowledges_concern": False},
            },
            id="record_2",
        ),
        GenAIEvalRecord(
            context={
                "metrics": {"quality_score": 5, "accuracy_score": 6},
                "response": {"provides_solution": False, "acknowledges_concern": True},
            },
            id="record_3",
        ),
        GenAIEvalRecord(
            context={
                "metrics": {"quality_score": 7, "accuracy_score": 8},
                "response": {"provides_solution": True, "acknowledges_concern": True},
            },
            id="record_4",
        ),
        GenAIEvalRecord(
            context={
                "metrics": {"quality_score": 4, "accuracy_score": 5},
                "response": {"provides_solution": False, "acknowledges_concern": False},
            },
            id="record_5",
        ),
    ]
