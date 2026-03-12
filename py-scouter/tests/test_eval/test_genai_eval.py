import pandas as pd
import polars as pl
from scouter._scouter import ComparisonOperator
from scouter.evaluate import (
    AssertionTask,
    EvalDataset,
    EvalRecord,
    EvaluationConfig,
    EvalResults,
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
        record = EvalRecord(
            context={"input": {"foo": "bar", "bar": 42, "baz": [1, 2, 3]}},
            id="test_id_1",
        )

        dataset = EvalDataset(
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
            record = EvalRecord(
                context={"input": {"foo": "bar", "bar": 42, "baz": [1, 2, 3]}},
                id=f"test_id_{i}",
            )
            records.append(record)

        dataset = EvalDataset(
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
            record = EvalRecord(
                context={"input": {"foo": "bar", "bar": 42, "baz": [1, 2, 3]}},
                id=f"test_id_{i}",
            )
            records.append(record)

        dataset = EvalDataset(
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
            record = EvalRecord(
                context={
                    "input": {"foo": "bar", "bar": 42, "baz": [1, 2, 3]},
                    "response": "my response",
                },
                id=f"test_id_{i}",
            )
            records.append(record)

        dataset = EvalDataset(
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
        assert isinstance(validated_results, EvalResults)

        histograms = results.histograms
        assert histograms is not None
        for field, histogram in histograms.items():
            print(f"Histogram for {field}: {histogram}")


# ── Array evaluation tests ────────────────────────────────────────────────────


def test_array_of_objects_not_empty_pass() -> None:
    """Array of objects is non-empty → PASS."""
    record = EvalRecord(
        context={"responses": [{"text": "hello"}, {"text": "world"}, {"text": "foo"}]},
        id="array_obj_not_empty_pass",
    )
    task = AssertionTask(
        id="responses_not_empty",
        context_path="responses",
        item_context_path="text",
        operator=ComparisonOperator.IsNotEmpty,
        expected_value=True,
        description="Responses array must have at least one entry",
    )
    dataset = EvalDataset(records=[record], tasks=[task])
    results = dataset.evaluate()
    assert results["array_obj_not_empty_pass"].eval_set.failed_tasks == 0


def test_array_of_objects_not_empty_fail() -> None:
    """Array of objects is empty (no items) → FAIL."""
    record = EvalRecord(
        context={"responses": []},
        id="array_obj_not_empty_fail",
    )
    task = AssertionTask(
        id="responses_not_empty",
        context_path="responses",
        operator=ComparisonOperator.IsNotEmpty,
        expected_value=True,
        description="Responses array must have at least one entry",
    )
    dataset = EvalDataset(records=[record], tasks=[task])
    results = dataset.evaluate()
    assert results["array_obj_not_empty_fail"].eval_set.failed_tasks > 0


def test_array_items_greater_than_pass() -> None:
    """All numeric values in the array are >= 5 → PASS."""
    record = EvalRecord(
        context={"scores": [8, 6, 10]},
        id="attr_gte_pass",
    )
    task = AssertionTask(
        id="score_gte_5",
        context_path="scores",
        operator=ComparisonOperator.GreaterThanOrEqual,
        expected_value=5,
        description="Every score must be >= 5",
    )
    dataset = EvalDataset(records=[record], tasks=[task])
    results = dataset.evaluate()
    assert results["attr_gte_pass"].eval_set.failed_tasks == 0


def test_array_items_greater_than_fail() -> None:
    """One numeric value in the array is < 5 → FAIL."""
    record = EvalRecord(
        context={"scores": [8, 3, 10]},
        id="attr_gte_fail",
    )
    task = AssertionTask(
        id="score_gte_5",
        context_path="scores",
        operator=ComparisonOperator.GreaterThanOrEqual,
        expected_value=5,
        description="Every score must be >= 5",
    )
    dataset = EvalDataset(records=[record], tasks=[task])
    results = dataset.evaluate()
    assert results["attr_gte_fail"].eval_set.failed_tasks > 0


def test_array_items_less_than_pass() -> None:
    """All numeric values in the array are < 500 → PASS."""
    record = EvalRecord(
        context={"latencies": [120, 200, 450]},
        id="attr_lt_pass",
    )
    task = AssertionTask(
        id="latency_lt_500",
        context_path="latencies",
        operator=ComparisonOperator.LessThan,
        expected_value=500,
        description="Every latency must be < 500ms",
    )
    dataset = EvalDataset(records=[record], tasks=[task])
    results = dataset.evaluate()
    assert results["attr_lt_pass"].eval_set.failed_tasks == 0


def test_array_items_less_than_fail() -> None:
    """One numeric value in the array is >= 500 → FAIL."""
    record = EvalRecord(
        context={"latencies": [120, 600, 450]},
        id="attr_lt_fail",
    )
    task = AssertionTask(
        id="latency_lt_500",
        context_path="latencies",
        operator=ComparisonOperator.LessThan,
        expected_value=500,
        description="Every latency must be < 500ms",
    )
    dataset = EvalDataset(records=[record], tasks=[task])
    results = dataset.evaluate()
    assert results["attr_lt_fail"].eval_set.failed_tasks > 0


def test_array_of_scalars_greater_than_pass() -> None:
    """All scalar values in the array are > 0 → PASS."""
    record = EvalRecord(
        context={"scores": [5, 8, 10, 7]},
        id="scalar_gt_pass",
    )
    task = AssertionTask(
        id="scores_positive",
        context_path="scores",
        operator=ComparisonOperator.GreaterThan,
        expected_value=0,
        description="Every score must be positive",
    )
    dataset = EvalDataset(records=[record], tasks=[task])
    results = dataset.evaluate()
    assert results["scalar_gt_pass"].eval_set.failed_tasks == 0


def test_array_of_scalars_greater_than_fail() -> None:
    """One scalar value is not > 0 → FAIL."""
    record = EvalRecord(
        context={"scores": [5, 8, -1, 7]},
        id="scalar_gt_fail",
    )
    task = AssertionTask(
        id="scores_positive",
        context_path="scores",
        operator=ComparisonOperator.GreaterThan,
        expected_value=0,
        description="Every score must be positive",
    )
    dataset = EvalDataset(records=[record], tasks=[task])
    results = dataset.evaluate()
    assert results["scalar_gt_fail"].eval_set.failed_tasks > 0


def test_array_of_scalars_less_than_pass() -> None:
    """All scalar values are < 100 → PASS."""
    record = EvalRecord(
        context={"percentages": [10, 45, 99, 72]},
        id="scalar_lt_pass",
    )
    task = AssertionTask(
        id="pct_lt_100",
        context_path="percentages",
        operator=ComparisonOperator.LessThan,
        expected_value=100,
        description="Every percentage must be < 100",
    )
    dataset = EvalDataset(records=[record], tasks=[task])
    results = dataset.evaluate()
    assert results["scalar_lt_pass"].eval_set.failed_tasks == 0


def test_array_of_scalars_less_than_fail() -> None:
    """One scalar value is >= 100 → FAIL."""
    record = EvalRecord(
        context={"percentages": [10, 45, 100, 72]},
        id="scalar_lt_fail",
    )
    task = AssertionTask(
        id="pct_lt_100",
        context_path="percentages",
        operator=ComparisonOperator.LessThan,
        expected_value=100,
        description="Every percentage must be < 100",
    )
    dataset = EvalDataset(records=[record], tasks=[task])
    results = dataset.evaluate()
    assert results["scalar_lt_fail"].eval_set.failed_tasks > 0


def test_array_native_has_length_unaffected() -> None:
    """HasLengthEqual operates on the array as a whole, not its items."""
    record = EvalRecord(
        context={"tags": ["a", "b", "c"]},
        id="has_length_native",
    )
    task = AssertionTask(
        id="tags_length",
        context_path="tags",
        operator=ComparisonOperator.HasLengthEqual,
        expected_value=3,
        description="Array must have exactly 3 items",
    )
    dataset = EvalDataset(records=[record], tasks=[task])
    results = dataset.evaluate()
    assert results["has_length_native"].eval_set.failed_tasks == 0


def test_genai_conditional_assertions():
    """Main goal of this test is to ensure that conditional assertions work as expected."""

    stage_1_tasks = [
        AssertionTask(
            id="is_foo",
            context_path="input",
            operator=ComparisonOperator.Equals,
            expected_value="foo",
            description="Check if input is 'foo'",
            condition=True,
        ),
        AssertionTask(
            id="is_bar",
            context_path="input",
            operator=ComparisonOperator.Equals,
            expected_value="bar",
            description="Check if input is 'bar'",
            condition=True,
        ),
        AssertionTask(
            id="is_baz",
            context_path="input",
            operator=ComparisonOperator.Equals,
            expected_value="baz",
            description="Check if input is 'baz'",
            condition=True,
        ),
    ]

    stage_2_task = [
        AssertionTask(
            id="is_foo_foo",
            context_path="response",
            operator=ComparisonOperator.Equals,
            expected_value="foo_foo",
            description="Check if response is 'foo_foo'",
            depends_on=["is_foo"],
        ),
        AssertionTask(
            id="is_bar_bar",
            context_path="response",
            operator=ComparisonOperator.Equals,
            expected_value="bar_bar",
            description="Check if response is 'bar_bar'",
            depends_on=["is_bar"],
        ),
        AssertionTask(
            id="is_baz_baz",
            context_path="response",
            operator=ComparisonOperator.Equals,
            expected_value="baz_baz",
            description="Check if response is 'baz_baz'",
            depends_on=["is_baz"],
        ),
    ]

    record = EvalRecord(
        context={"input": "bar", "response": "bar_bar"},
        id="test_conditional_1",
    )

    dataset = EvalDataset(
        records=[record],
        tasks=stage_1_tasks + stage_2_task,
    )

    results = dataset.evaluate()

    results.as_table()

    assert results["test_conditional_1"].task_count == 4, (
        f"Expected 2 tasks to run, got {results['test_conditional_1'].task_count}"
    )
    assert results["test_conditional_1"].eval_set.records[0].task_id == "is_foo", (
        f"Expected first task to be 'is_bar', got {results['test_conditional_1'].eval_set.records[0].task_id}"
    )
