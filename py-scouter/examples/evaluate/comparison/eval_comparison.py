"""
Evaluation comparison: baseline vs improved agent.

This example shows how to compare two versions of an agent against the same
set of scenarios to detect regressions or confirm improvements:

  1. Run the baseline agent — save results to baseline_results.json.
  2. Run the improved agent — save results to improved_results.json.
  3. Load both from disk and call compare_to() to get a comparison report.

Saving results to JSON means each evaluation run can happen independently
(different processes, different times, different environments) and the
comparison step only needs the saved files.

Replace GrpcConfig() and the simulated agent_fn functions with your real setup.
"""

import os

from scouter.drift import GenAIEvalProfile
from scouter.evaluate import (
    AssertionTask,
    ComparisonOperator,
    EvalOrchestrator,
    EvalRecord,
    EvalScenario,
    EvalScenarios,
    ScenarioComparisonResults,
    ScenarioEvalResults,
)
from scouter.queue import ScouterQueue
from scouter.tracing import init_tracer
from scouter.transport import GrpcConfig

BASELINE_PATH = "baseline_results.json"
IMPROVED_PATH = "improved_results.json"

# ---------------------------------------------------------------------------
# 1. Shared profile and scenarios — both runs use exactly the same definition.
#    This is what makes the comparison valid: same criteria, same inputs.
# ---------------------------------------------------------------------------
profile = GenAIEvalProfile(
    alias="product_agent",
    tasks=[
        AssertionTask(
            id="response_is_string",
            context_path="response",
            operator=ComparisonOperator.IsString,
            expected_value=True,
        ),
        AssertionTask(
            id="confidence_threshold",
            context_path="confidence",
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=0.75,
        ),
    ],
)

scenarios = EvalScenarios(
    scenarios=[
        EvalScenario(
            id="laptop_query",
            initial_query="What laptops do you have under $1000?",
            expected_outcome="A list of laptop options with prices",
            tasks=[
                AssertionTask(
                    id="mentions_price",
                    context_path="response",
                    operator=ComparisonOperator.Contains,
                    expected_value="$",
                ),
            ],
        ),
        EvalScenario(
            id="return_policy",
            initial_query="What is your return policy?",
            expected_outcome="Clear explanation of the return window and process",
            tasks=[
                AssertionTask(
                    id="mentions_days",
                    context_path="response",
                    operator=ComparisonOperator.Contains,
                    expected_value="day",
                ),
            ],
        ),
        EvalScenario(
            id="warranty_question",
            initial_query="Do your electronics come with a warranty?",
            expected_outcome="Details on warranty coverage and duration",
            tasks=[
                AssertionTask(
                    id="mentions_warranty",
                    context_path="response",
                    operator=ComparisonOperator.Contains,
                    expected_value="warranty",
                ),
            ],
        ),
        EvalScenario(
            id="shipping_time",
            initial_query="How long does shipping take?",
            expected_outcome="Expected delivery timeframe",
            tasks=[
                AssertionTask(
                    id="mentions_days",
                    context_path="response",
                    operator=ComparisonOperator.Contains,
                    expected_value="day",
                ),
            ],
        ),
    ]
)

# ---------------------------------------------------------------------------
# 2. Queue and tracer — shared by both runs.
# ---------------------------------------------------------------------------
queue = ScouterQueue.from_profile(
    profile=[profile],
    transport_config=GrpcConfig(),
)

tracer = init_tracer(
    service_name="product-agent",
    scouter_queue=queue,
    transport_config=GrpcConfig(),
)


# ---------------------------------------------------------------------------
# 3. Two versions of the agent.
#
#    Baseline: terse answers, low confidence — some assertions will fail.
#    Improved: detailed answers, high confidence — all assertions pass.
#
#    In a real comparison you would import v1 and v2 of your agent here.
# ---------------------------------------------------------------------------
_BASELINE_DATA = {
    "What laptops do you have under $1000?": (
        "We have several laptops available.",
        0.55,  # below the 0.75 threshold — will FAIL
    ),
    "What is your return policy?": (
        "You can return items. Contact support.",
        0.60,  # below threshold — will FAIL
    ),
    "Do your electronics come with a warranty?": (
        "Yes, our electronics have a warranty.",
        0.70,  # below threshold — will FAIL
    ),
    "How long does shipping take?": (
        "Shipping takes a few days.",
        0.65,  # below threshold — will FAIL
    ),
}

_IMPROVED_DATA = {
    "What laptops do you have under $1000?": (
        "We carry 12 laptops priced from $499–$999, including the Dell XPS 13 at $799 and the HP Spectre at $949.",
        0.92,
    ),
    "What is your return policy?": (
        "You can return any item within 30 days of delivery for a full refund. Items must be in original condition.",
        0.95,
    ),
    "Do your electronics come with a warranty?": (
        "All electronics include a 1-year manufacturer warranty. Extended warranty options (2 or 3 years) are available at checkout.",
        0.93,
    ),
    "How long does shipping take?": (
        "Standard shipping takes 5–7 business days. Expedited options (2-day, overnight) are available at checkout.",
        0.96,
    ),
}


def _make_agent_fn(data: dict[str, tuple[str, float]]):
    """Factory that wraps a data lookup as an agent_fn."""

    def agent_fn(query: str) -> str:
        response, confidence = data.get(query, ("I'm not sure.", 0.3))

        with tracer.start_as_current_span("product_agent_call") as span:
            span.add_queue_item(
                "product_agent",
                EvalRecord(
                    context={"response": response, "confidence": confidence},
                    id=f"product_{abs(hash(query)) % 10_000}",
                ),
            )

        return response

    return agent_fn


# ---------------------------------------------------------------------------
# 4. Run baseline, save results to disk.
# ---------------------------------------------------------------------------
def run_baseline() -> ScenarioEvalResults:
    print("\n=== Baseline Agent ===\n")
    results = EvalOrchestrator(
        queue=queue,
        scenarios=scenarios,
        agent_fn=_make_agent_fn(_BASELINE_DATA),
    ).run()
    results.save(BASELINE_PATH)
    print(f"  Saved → {BASELINE_PATH}")
    results.as_table(show_datasets=True)
    return results


# ---------------------------------------------------------------------------
# 5. Run improved, save results to disk.
# ---------------------------------------------------------------------------
def run_improved() -> ScenarioEvalResults:
    print("\n=== Improved Agent ===\n")
    results = EvalOrchestrator(
        queue=queue,
        scenarios=scenarios,
        agent_fn=_make_agent_fn(_IMPROVED_DATA),
    ).run()
    results.save(IMPROVED_PATH)
    print(f"  Saved → {IMPROVED_PATH}")
    results.as_table(show_datasets=True)
    return results


# ---------------------------------------------------------------------------
# 6. Load from disk and compare.
#    compare_to() works with files saved in prior runs — runs can be in
#    separate scripts or CI pipelines as long as the JSON files are available.
# ---------------------------------------------------------------------------
def compare() -> ScenarioComparisonResults:
    print("\n=== Comparison Analysis ===\n")
    baseline = ScenarioEvalResults.load(BASELINE_PATH)
    improved = ScenarioEvalResults.load(IMPROVED_PATH)

    comparison = improved.compare_to(baseline)
    comparison.as_table()

    print(f"\n  Baseline pass rate : {comparison.baseline_overall_pass_rate:.0%}")
    print(f"  Improved pass rate : {comparison.comparison_overall_pass_rate:.0%}")
    print(f"  Regressed          : {comparison.regressed}")

    return comparison


def main() -> None:
    run_baseline()
    run_improved()
    compare()

    # Clean up output files after the example
    os.remove(BASELINE_PATH)
    os.remove(IMPROVED_PATH)


if __name__ == "__main__":
    main()
