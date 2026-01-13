from scouter.evaluate import (
    AssertionTask,
    ComparisonOperator,
    GenAIEvalDataset,
    GenAIEvalRecord,
)


def test_comparison_improvement_detected(base_assertion_tasks, baseline_records, improved_records):
    """Test that improvements are correctly detected in comparison."""
    baseline_dataset = GenAIEvalDataset(records=baseline_records, tasks=base_assertion_tasks)
    improved_dataset = GenAIEvalDataset(records=improved_records, tasks=base_assertion_tasks)

    baseline_results = baseline_dataset.evaluate()
    improved_results = improved_dataset.evaluate()

    comparison = improved_results.compare_to(
        baseline=baseline_results,
        regression_threshold=0.05,
    )

    assert comparison.improved_workflows > 0, "Expected to detect improvements"
    assert comparison.regressed_workflows == 0, "No regressions should be detected"
    assert comparison.mean_pass_rate_delta > 0, f"Success rate should improve, got {comparison.mean_pass_rate_delta}"


def test_comparison_regression_detected(base_assertion_tasks, baseline_records, regressed_records):
    """Test that regressions are correctly detected in comparison."""
    baseline_dataset = GenAIEvalDataset(records=baseline_records, tasks=base_assertion_tasks)
    regressed_dataset = GenAIEvalDataset(records=regressed_records, tasks=base_assertion_tasks)

    baseline_results = baseline_dataset.evaluate()
    regressed_results = regressed_dataset.evaluate()

    comparison = regressed_results.compare_to(
        baseline=baseline_results,
        regression_threshold=0.05,
    )

    assert comparison.regressed_workflows > 0, "Expected to detect regressions"
    assert comparison.mean_pass_rate_delta < 0, f"Success rate should decrease, got {comparison.mean_pass_rate_delta}"


def test_comparison_no_change(base_assertion_tasks, baseline_records):
    """Test comparison when results are identical."""
    dataset1 = GenAIEvalDataset(records=baseline_records, tasks=base_assertion_tasks)
    dataset2 = GenAIEvalDataset(records=baseline_records, tasks=base_assertion_tasks)

    results1 = dataset1.evaluate()
    results2 = dataset2.evaluate()

    comparison = results2.compare_to(
        baseline=results1,
        regression_threshold=0.05,
    )

    comparison.as_table()

    assert comparison.improved_workflows == 0, "No improvements should be detected"
    assert comparison.regressed_workflows == 0, "No regressions should be detected"
    assert (
        abs(comparison.mean_pass_rate_delta) < 0.01
    ), f"Success rate should be stable, got {comparison.mean_pass_rate_delta}"


def test_comparison_with_updated_contexts(base_assertion_tasks, baseline_records):
    """Test comparison using with_updated_contexts_by_id method."""
    baseline_dataset = GenAIEvalDataset(records=baseline_records, tasks=base_assertion_tasks)
    baseline_results = baseline_dataset.evaluate()

    context_updates = {
        "record_1": {
            "metrics": {"quality_score": 9, "accuracy_score": 10},
            "response": {"provides_solution": True, "acknowledges_concern": True},
        },
        "record_3": {
            "metrics": {"quality_score": 8, "accuracy_score": 9},
            "response": {"provides_solution": True, "acknowledges_concern": True},
        },
        "record_5": {
            "metrics": {"quality_score": 7, "accuracy_score": 8},
            "response": {"provides_solution": True, "acknowledges_concern": True},
        },
    }

    improved_dataset = baseline_dataset.with_updated_contexts_by_id(context_updates)
    improved_results = improved_dataset.evaluate()

    comparison = improved_results.compare_to(
        baseline=baseline_results,
        regression_threshold=0.05,
    )

    comparison.as_table()

    assert comparison.improved_workflows > 0, "Expected improvements in updated records"
    assert (
        comparison.mean_pass_rate_delta > 0
    ), f"Overall success rate should improve, got {comparison.mean_pass_rate_delta}"


def test_comparison_threshold_sensitivity(base_assertion_tasks, baseline_records):
    """Test that regression threshold affects comparison results."""
    baseline_dataset = GenAIEvalDataset(records=baseline_records, tasks=base_assertion_tasks)
    baseline_results = baseline_dataset.evaluate()

    context_updates = {
        "record_1": {
            "metrics": {"quality_score": 6, "accuracy_score": 7},
        },
    }

    slightly_worse_dataset = baseline_dataset.with_updated_contexts_by_id(context_updates)
    slightly_worse_results = slightly_worse_dataset.evaluate()

    strict_comparison = slightly_worse_results.compare_to(
        baseline=baseline_results,
        regression_threshold=0.01,
    )

    lenient_comparison = slightly_worse_results.compare_to(
        baseline=baseline_results,
        regression_threshold=0.20,
    )

    assert (
        strict_comparison.regressed_workflows >= lenient_comparison.regressed_workflows
    ), "Strict threshold should detect more regressions"


def test_comparison_with_conditional_tasks():
    """Test comparison with conditional assertion tasks."""
    conditional_tasks = [
        AssertionTask(
            id="is_premium",
            field_path="customer.tier",
            operator=ComparisonOperator.Equals,
            expected_value="premium",
            description="Check if customer is premium",
            condition=True,
        ),
        AssertionTask(
            id="premium_response_quality",
            field_path="metrics.quality_score",
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=9,
            description="Premium customers require quality >= 9",
            depends_on=["is_premium"],
        ),
        AssertionTask(
            id="is_standard",
            field_path="customer.tier",
            operator=ComparisonOperator.Equals,
            expected_value="standard",
            description="Check if customer is standard",
            condition=True,
        ),
        AssertionTask(
            id="standard_response_quality",
            field_path="metrics.quality_score",
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=7,
            description="Standard customers require quality >= 7",
            depends_on=["is_standard"],
        ),
    ]

    baseline_records = [
        GenAIEvalRecord(
            context={
                "customer": {"tier": "premium"},
                "metrics": {"quality_score": 8},
            },
            id="premium_1",
        ),
        GenAIEvalRecord(
            context={
                "customer": {"tier": "standard"},
                "metrics": {"quality_score": 6},
            },
            id="standard_1",
        ),
    ]

    improved_records = [
        GenAIEvalRecord(
            context={
                "customer": {"tier": "premium"},
                "metrics": {"quality_score": 10},
            },
            id="premium_1",
        ),
        GenAIEvalRecord(
            context={
                "customer": {"tier": "standard"},
                "metrics": {"quality_score": 8},
            },
            id="standard_1",
        ),
    ]

    baseline_dataset = GenAIEvalDataset(records=baseline_records, tasks=conditional_tasks)
    improved_dataset = GenAIEvalDataset(records=improved_records, tasks=conditional_tasks)

    baseline_results = baseline_dataset.evaluate()
    improved_results = improved_dataset.evaluate()

    comparison = improved_results.compare_to(
        baseline=baseline_results,
        regression_threshold=0.05,
    )

    assert comparison.improved_workflows > 0, "Expected improvements with conditional tasks"
    assert comparison.mean_pass_rate_delta > 0, f"Success rate should improve, got {comparison.mean_pass_rate_delta}"


def test_comparison_mixed_results():
    """Test comparison where some workflows improve and others regress."""
    tasks = [
        AssertionTask(
            id="quality_check",
            field_path="score",
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=7,
            description="Score must be >= 7",
        ),
    ]
    baseline_records = [
        GenAIEvalRecord(context={"score": 5}, id="workflow_1"),
        GenAIEvalRecord(context={"score": 8}, id="workflow_2"),
        GenAIEvalRecord(context={"score": 6}, id="workflow_3"),
        GenAIEvalRecord(context={"score": 9}, id="workflow_4"),
    ]

    mixed_records = [
        GenAIEvalRecord(context={"score": 8}, id="workflow_1"),
        GenAIEvalRecord(context={"score": 5}, id="workflow_2"),
        GenAIEvalRecord(context={"score": 7}, id="workflow_3"),
        GenAIEvalRecord(context={"score": 10}, id="workflow_4"),
    ]

    baseline_dataset = GenAIEvalDataset(records=baseline_records, tasks=tasks)
    mixed_dataset = GenAIEvalDataset(records=mixed_records, tasks=tasks)

    baseline_results = baseline_dataset.evaluate()
    mixed_results = mixed_dataset.evaluate()

    comparison = mixed_results.compare_to(
        baseline=baseline_results,
        regression_threshold=0.05,
    )

    comparison.as_table()

    assert comparison.improved_workflows > 0, "Should detect improved workflows"
    assert comparison.regressed_workflows > 0, "Should detect regressed workflows"
