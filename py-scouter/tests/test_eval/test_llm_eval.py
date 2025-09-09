import pandas as pd
import polars as pl
from scouter.evaluate import (
    LLMEvalMetric,
    LLMEvalRecord,
    evaluate_llm,
    EvaluationConfig,
)  # type: ignore
from scouter.llm import Embedder, Provider  # type: ignore
from scouter.llm.openai import OpenAIEmbeddingConfig  # type: ignore
from scouter.mock import LLMTestServer


def _test_llm_eval_no_embedding(
    reformulation_evaluation_prompt, relevancy_evaluation_prompt
) -> None:
    with LLMTestServer():
        records = []
        for i in range(10):
            record = LLMEvalRecord(
                context={"user_query": "my query", "response": "my response"},
                id=f"test_id_{i}",
            )
            records.append(record)

        reformulation_metric = LLMEvalMetric(
            name="reformulation",
            prompt=reformulation_evaluation_prompt,
        )
        relevancy_metric = LLMEvalMetric(
            name="relevancy",
            prompt=relevancy_evaluation_prompt,
        )
        results = evaluate_llm(
            records=records,
            metrics=[reformulation_metric, relevancy_metric],
        )

        for result in results:
            assert result["reformulation"].score > 0
            assert result["relevancy"].score > 0

        result_df: pd.DataFrame = results.to_dataframe()

        assert isinstance(result_df, pd.DataFrame)

        result_polars_df: pl.DataFrame = results.to_dataframe(polars=True)

        assert isinstance(result_polars_df, pl.DataFrame)


def test_llm_eval_embedding(
    reformulation_evaluation_prompt, relevancy_evaluation_prompt
) -> None:
    with LLMTestServer():
        records = []

        embedder = Embedder(
            Provider.OpenAI,
            config=OpenAIEmbeddingConfig(
                model="text-embedding-3-small",
                dimensions=512,
            ),
        )
        for i in range(10):
            record = LLMEvalRecord(
                context={"user_query": "my query", "response": "my response"},
                id=f"test_id_{i}",
            )
            records.append(record)

        reformulation_metric = LLMEvalMetric(
            name="reformulation",
            prompt=reformulation_evaluation_prompt,
        )
        relevancy_metric = LLMEvalMetric(
            name="relevancy",
            prompt=relevancy_evaluation_prompt,
        )
        results = evaluate_llm(
            records=records,
            metrics=[reformulation_metric, relevancy_metric],
            config=EvaluationConfig(
                embedder=embedder,
                embedding_targets=["user_query"],
            ),
        )

        assert results["test_id_1"]["reformulation"].score > 0
        assert results["test_id_1"]["relevancy"].score > 0

        result_df: pd.DataFrame = results.to_dataframe()

        assert isinstance(result_df, pd.DataFrame)

        result_polars_df: pl.DataFrame = results.to_dataframe(polars=True)

        assert isinstance(result_polars_df, pl.DataFrame)

        assert result_df.shape[0] == 20  # 10 records x 2 metrics
        a
