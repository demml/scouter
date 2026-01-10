"""
Demonstrates GenAI evaluation comparison functionality.

This example generates baseline and comparison evaluation runs for a product
categorization system, then compares them to detect improvements and regressions.
Uses only assertion-based tasks to showcase objective metric tracking across runs.
"""

from typing import List

from pydantic import BaseModel
from scouter.evaluate import (
    AssertionTask,
    ComparisonOperator,
    EvaluationConfig,
    GenAIEvalDataset,
)
from scouter.logging import LoggingConfig, LogLevel, RustyLogger
from scouter.queue import GenAIEvalRecord

RustyLogger.setup_logging(LoggingConfig(log_level=LogLevel.Info))


class ProductClassification(BaseModel):
    """Simulated ML model output for product categorization."""

    category: str
    confidence: float
    processing_time_ms: int
    feature_count: int
    matched_keywords: List[str]


class ModelOutput(BaseModel):
    """Ground truth and model prediction structure for product classification."""

    ground_truth: str
    prediction: ProductClassification


def generate_baseline_classifications() -> List[ModelOutput]:
    """
    Simulates baseline model predictions with some correct and incorrect classifications.
    Returns list of (ground_truth, prediction) tuples.
    """
    return [
        ModelOutput(
            ground_truth="electronics",
            prediction=ProductClassification(
                category="electronics",
                confidence=0.92,
                processing_time_ms=45,
                feature_count=8,
                matched_keywords=["phone", "screen", "battery"],
            ),
        ),
        ModelOutput(
            ground_truth="clothing",
            prediction=ProductClassification(
                category="clothing",
                confidence=0.88,
                processing_time_ms=38,
                feature_count=6,
                matched_keywords=["cotton", "shirt", "size"],
            ),
        ),
        ModelOutput(
            ground_truth="home_goods",
            prediction=ProductClassification(
                category="furniture",
                confidence=0.65,
                processing_time_ms=52,
                feature_count=5,
                matched_keywords=["table", "wood"],
            ),
        ),
        ModelOutput(
            ground_truth="electronics",
            prediction=ProductClassification(
                category="electronics",
                confidence=0.95,
                processing_time_ms=42,
                feature_count=9,
                matched_keywords=["laptop", "processor", "ram"],
            ),
        ),
        ModelOutput(
            ground_truth="books",
            prediction=ProductClassification(
                category="books",
                confidence=0.91,
                processing_time_ms=35,
                feature_count=7,
                matched_keywords=["paperback", "author", "pages"],
            ),
        ),
        ModelOutput(
            ground_truth="clothing",
            prediction=ProductClassification(
                category="accessories",
                confidence=0.72,
                processing_time_ms=48,
                feature_count=4,
                matched_keywords=["leather", "belt"],
            ),
        ),
        ModelOutput(
            ground_truth="home_goods",
            prediction=ProductClassification(
                category="home_goods",
                confidence=0.89,
                processing_time_ms=41,
                feature_count=6,
                matched_keywords=["lamp", "decor", "light"],
            ),
        ),
        ModelOutput(
            ground_truth="electronics",
            prediction=ProductClassification(
                category="electronics",
                confidence=0.93,
                processing_time_ms=44,
                feature_count=8,
                matched_keywords=["tablet", "touchscreen", "wifi"],
            ),
        ),
        ModelOutput(
            ground_truth="books",
            prediction=ProductClassification(
                category="media",
                confidence=0.68,
                processing_time_ms=55,
                feature_count=5,
                matched_keywords=["fiction", "novel"],
            ),
        ),
        ModelOutput(
            ground_truth="clothing",
            prediction=ProductClassification(
                category="clothing",
                confidence=0.90,
                processing_time_ms=37,
                feature_count=7,
                matched_keywords=["denim", "jeans", "pants"],
            ),
        ),
    ]


def generate_improved_classifications() -> List[ModelOutput]:
    """
    Simulates improved model predictions with better accuracy and performance.
    Fixes some baseline errors and improves metrics.
    """
    return [
        ModelOutput(
            ground_truth="electronics",
            prediction=ProductClassification(
                category="electronics",
                confidence=0.94,
                processing_time_ms=38,
                feature_count=10,
                matched_keywords=["phone", "screen", "battery", "5g"],
            ),
        ),
        ModelOutput(
            ground_truth="clothing",
            prediction=ProductClassification(
                category="clothing",
                confidence=0.91,
                processing_time_ms=32,
                feature_count=8,
                matched_keywords=["cotton", "shirt", "size", "fabric"],
            ),
        ),
        ModelOutput(
            ground_truth="home_goods",
            prediction=ProductClassification(
                category="home_goods",
                confidence=0.87,
                processing_time_ms=40,
                feature_count=7,
                matched_keywords=["table", "wood", "furniture", "decor"],
            ),
        ),
        ModelOutput(
            ground_truth="electronics",
            prediction=ProductClassification(
                category="electronics",
                confidence=0.96,
                processing_time_ms=35,
                feature_count=11,
                matched_keywords=["laptop", "processor", "ram", "ssd"],
            ),
        ),
        ModelOutput(
            ground_truth="books",
            prediction=ProductClassification(
                category="books",
                confidence=0.93,
                processing_time_ms=30,
                feature_count=9,
                matched_keywords=["paperback", "author", "pages", "isbn"],
            ),
        ),
        ModelOutput(
            ground_truth="clothing",
            prediction=ProductClassification(
                category="clothing",
                confidence=0.85,
                processing_time_ms=36,
                feature_count=6,
                matched_keywords=["leather", "belt", "accessory", "fashion"],
            ),
        ),
        ModelOutput(
            ground_truth="home_goods",
            prediction=ProductClassification(
                category="home_goods",
                confidence=0.92,
                processing_time_ms=34,
                feature_count=8,
                matched_keywords=["lamp", "decor", "light", "interior"],
            ),
        ),
        ModelOutput(
            ground_truth="electronics",
            prediction=ProductClassification(
                category="electronics",
                confidence=0.95,
                processing_time_ms=36,
                feature_count=10,
                matched_keywords=["tablet", "touchscreen", "wifi", "apps"],
            ),
        ),
        ModelOutput(
            ground_truth="books",
            prediction=ProductClassification(
                category="books",
                confidence=0.88,
                processing_time_ms=42,
                feature_count=7,
                matched_keywords=["fiction", "novel", "author", "paperback"],
            ),
        ),
        ModelOutput(
            ground_truth="clothing",
            prediction=ProductClassification(
                category="clothing",
                confidence=0.92,
                processing_time_ms=31,
                feature_count=9,
                matched_keywords=["denim", "jeans", "pants", "cotton"],
            ),
        ),
    ]


def create_baseline_dataset() -> GenAIEvalDataset:
    """
    Creates baseline evaluation dataset with stable record IDs.
    Uses record IDs without prefixes so they can be matched across runs.
    """
    baseline_data = generate_baseline_classifications()

    records = []
    for idx, model_output in enumerate(baseline_data):
        record = GenAIEvalRecord(context=model_output, id=f"product_classification_{idx}")
        records.append(record)

    tasks = [
        AssertionTask(
            id="correct_category",
            field_path="prediction.category",
            operator=ComparisonOperator.Equals,
            expected_value="${ground_truth}",
            description="Verify predicted category matches ground truth",
        ),
        AssertionTask(
            id="high_confidence",
            field_path="prediction.confidence",
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=0.85,
            description="Verify model confidence is >= 0.85",
        ),
        AssertionTask(
            id="fast_processing",
            field_path="prediction.processing_time_ms",
            operator=ComparisonOperator.LessThanOrEqual,
            expected_value=50,
            description="Verify processing time is <= 50ms",
        ),
        AssertionTask(
            id="sufficient_features",
            field_path="prediction.feature_count",
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=6,
            description="Verify at least 6 features extracted",
        ),
        AssertionTask(
            id="has_keywords",
            field_path="prediction.matched_keywords",
            operator=ComparisonOperator.HasLengthGreaterThan,
            expected_value=2,
            description="Verify at least 3 keywords matched",
        ),
    ]

    return GenAIEvalDataset(records=records, tasks=tasks)


def main():
    """Run baseline and comparison evaluations, then compare results."""

    print("\n" + "=" * 80)
    print("BASELINE MODEL EVALUATION")
    print("=" * 80)

    baseline_dataset = create_baseline_dataset()
    baseline_dataset.print_execution_plan()
    baseline_results = baseline_dataset.evaluate()

    print("\n📊 Baseline Results:\n")
    baseline_results.as_table()

    print("\n" + "=" * 80)
    print("IMPROVED MODEL EVALUATION")
    print("=" * 80)

    improved_data = generate_improved_classifications()

    context_map = {f"product_classification_{idx}": model_output for idx, model_output in enumerate(improved_data)}
    improved_dataset = baseline_dataset.with_updated_contexts_by_id(context_map)

    improved_results = improved_dataset.evaluate()

    print("\n📊 Improved Results:\n")
    improved_results.as_table()

    print("\n" + "=" * 80)
    print("COMPARISON ANALYSIS")
    print("=" * 80)

    comparison = improved_results.compare_to(
        baseline=baseline_results,
        regression_threshold=0.05,
    )

    comparison.as_table()

    if comparison.regressed_workflows > 0:
        print("\n⚠️  REGRESSION DETECTED - Review failed workflows")
    elif comparison.improved_workflows > 0:
        print("\n✅ MODEL IMPROVEMENT CONFIRMED")
    else:
        print("\n➡️  No significant change detected")


if __name__ == "__main__":
    main()
