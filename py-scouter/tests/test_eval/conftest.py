import pytest
from scouter.evaluate import AssertionTask, ComparisonOperator, LLMJudgeTask
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
