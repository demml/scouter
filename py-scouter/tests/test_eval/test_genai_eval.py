import pandas as pd
import polars as pl
from scouter.evaluate import (
    EvaluationConfig,
    GenAIEvalDataset,
    GenAIEvalRecord,
    GenAIEvalResults,
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
