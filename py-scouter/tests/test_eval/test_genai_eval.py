import pandas as pd
import polars as pl
from scouter._scouter import ComparisonOperator
from scouter.evaluate import (
    EvaluationConfig,
    GenAIEvalDataset,
    GenAIEvalRecord,
    GenAIEvalResults,
    AssertionTask,
)
from scouter.genai import Embedder, Provider
from scouter.genai.openai import OpenAIEmbeddingConfig
from scouter.mock import LLMTestServer


# maintain parity with rust tests
def test_genai_eval_no_embedding(
    assertion_task_foo,
    llm_judge_query_relevance,
    query_relevance_score_assertion_task,
    query_relevance_reason_assertion_task,
) -> None:
    with LLMTestServer():
        record = GenAIEvalRecord(
            context={"input": {"foo": "bar", "bar": 42, "baz": [1, 2, 3]}},
            id="test_id_1",
        )

        dataset = GenAIEvalDataset(
            records=[record],
            tasks=[
                assertion_task_foo,
                llm_judge_query_relevance,
                query_relevance_score_assertion_task,
                query_relevance_reason_assertion_task,
            ],
        )

        assert len(dataset.llm_judge_tasks) == 1
        assert len(dataset.assertion_tasks) == 3

        results = dataset.evaluate()

        results.as_table()

        result_df: pd.DataFrame = results.to_dataframe()

        assert isinstance(result_df, pd.DataFrame)

        df: pl.DataFrame = results.to_dataframe(polars=True)

        assert isinstance(df, pl.DataFrame)


def test_genai_eval_no_embedding_one_fail(
    assertion_task_foo,
    llm_judge_query_relevance,
    query_relevance_score_assertion_task,
    query_relevance_reason_assertion_task,
    assertion_task_baz,
) -> None:
    with LLMTestServer():
        records = []

        for i in range(5):
            record = GenAIEvalRecord(
                context={"input": {"foo": "bar", "bar": 42, "baz": [1, 2, 3]}},
                id=f"test_id_{i}",
            )
            records.append(record)

        dataset = GenAIEvalDataset(
            records=records,
            tasks=[
                assertion_task_foo,
                assertion_task_baz,
                llm_judge_query_relevance,
                query_relevance_score_assertion_task,
                query_relevance_reason_assertion_task,
            ],
        )

        dataset.print_execution_plan()

        results = dataset.evaluate()
        results.as_table()

        assert len(dataset.llm_judge_tasks) == 1
        assert len(dataset.assertion_tasks) == 4


def test_genai_eval_no_embedding_all_assertion(
    assertion_task_foo,
    assertion_task_bar,
    assertion_task_baz,
) -> None:
    with LLMTestServer():
        records = []

        for i in range(5):
            record = GenAIEvalRecord(
                context={"input": {"foo": "bar", "bar": 42, "baz": [1, 2, 3]}},
                id=f"test_id_{i}",
            )
            records.append(record)

        dataset = GenAIEvalDataset(
            records=records,
            tasks=[
                assertion_task_foo,
                assertion_task_baz,
                assertion_task_bar,
            ],
        )

        dataset.print_execution_plan()

        results = dataset.evaluate()
        results.as_table()

        assert len(dataset.assertion_tasks) == 3


def test_genai_eval_embedding_all_assertion(
    assertion_task_foo,
    assertion_task_bar,
    assertion_task_baz,
) -> None:
    with LLMTestServer():
        embedder = Embedder(
            Provider.OpenAI,
            config=OpenAIEmbeddingConfig(
                model="text-embedding-3-small",
                dimensions=512,
            ),
        )

        records = []

        for i in range(5):
            record = GenAIEvalRecord(
                context={
                    "input": {"foo": "bar", "bar": 42, "baz": [1, 2, 3]},
                    "response": "my response",
                },
                id=f"test_id_{i}",
            )
            records.append(record)

        dataset = GenAIEvalDataset(
            records=records,
            tasks=[
                assertion_task_foo,
                assertion_task_baz,
                assertion_task_bar,
            ],
        )

        dataset.print_execution_plan()

        results = dataset.evaluate(
            config=EvaluationConfig(
                embedder=embedder,
                embedding_targets=["input.foo", "response"],
                compute_similarity=True,
                compute_histograms=True,
            ),
        )
        results.as_table()

        assert len(dataset.assertion_tasks) == 3

        json_str = results.model_dump_json()
        assert isinstance(json_str, str)

        validated_results = results.model_validate_json(json_str)
        assert isinstance(validated_results, GenAIEvalResults)

        histograms = results.histograms
        assert histograms is not None
        for field, histogram in histograms.items():
            print(f"Histogram for {field}: {histogram}")


def test_genai_conditional_assertions():
    """Main goal of this test is to ensure that conditional assertions work as expected."""

    stage_1_tasks = [
        AssertionTask(
            id="is_foo",
            field_path="input",
            operator=ComparisonOperator.Equals,
            expected_value="foo",
            description="Check if input is 'foo'",
            condition=True,
        ),
        AssertionTask(
            id="is_bar",
            field_path="input",
            operator=ComparisonOperator.Equals,
            expected_value="bar",
            description="Check if input is 'bar'",
            condition=True,
        ),
        AssertionTask(
            id="is_baz",
            field_path="input",
            operator=ComparisonOperator.Equals,
            expected_value="baz",
            description="Check if input is 'baz'",
            condition=True,
        ),
    ]

    stage_2_task = [
        AssertionTask(
            id="is_foo_foo",
            field_path="response",
            operator=ComparisonOperator.Equals,
            expected_value="foo_foo",
            description="Check if response is 'foo_foo'",
            depends_on=["is_foo"],
        ),
        AssertionTask(
            id="is_bar_bar",
            field_path="response",
            operator=ComparisonOperator.Equals,
            expected_value="bar_bar",
            description="Check if response is 'bar_bar'",
            depends_on=["is_bar"],
        ),
        AssertionTask(
            id="is_baz_baz",
            field_path="response",
            operator=ComparisonOperator.Equals,
            expected_value="baz_baz",
            description="Check if response is 'baz_baz'",
            depends_on=["is_baz"],
        ),
    ]

    record = GenAIEvalRecord(
        context={"input": "bar", "response": "bar_bar"},
        id="test_conditional_1",
    )

    dataset = GenAIEvalDataset(
        records=[record],
        tasks=stage_1_tasks + stage_2_task,
    )

    results = dataset.evaluate()

    results.as_table()

    print(results)

    assert results["test_conditional_1"].task_count == 2, (
        f"Expected 2 tasks to run, got {results['test_conditional_1'].task_count}"
    )
    assert results["test_conditional_1"].eval_set.records[0].task_id == "is_bar", (
        f"Expected first task to be 'is_bar', got {results['test_conditional_1'].eval_set.records[0].task_id}"
    )
