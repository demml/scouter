"""Mock interactive eval — full harness run with no LLM cost.

Mirrors examples/evaluate/interactive/google/evaluate.py exactly,
swapping GoogleAgentService for MockAgentService. Use this to verify
the eval harness (span capture, EvalRecord routing, task evaluation,
result rendering) before running against a live model.
"""

from __future__ import annotations

from typing import Any

from scouter.evaluate import EvalOrchestrator, EvalScenario

from ..shared import get_shared_config, teardown
from .agent import build_agent_service


def simulated_user_turn(
    initial_query: str,
    agent_response: Any,
    history: list[dict[str, Any]],
) -> str:
    del initial_query

    if len(history) >= 2:
        return "DONE"

    response_text = str(agent_response).lower()
    if "step" not in response_text:
        return "Give me concrete step-by-step actions and end with DONE when complete."
    return "Add one risk to watch for and then return DONE."


class MockEvalOrchestrator(EvalOrchestrator):
    def __init__(self) -> None:
        config = get_shared_config()
        super().__init__(
            queue=config.queue,
            scenarios=config.scenarios,
            simulated_user_fn=simulated_user_turn,
        )
        self._service = build_agent_service()

    def execute_agent_turn(self, scenario: EvalScenario, message: str) -> str:
        del scenario
        return self._service.run(message)


def main() -> None:
    orchestrator = MockEvalOrchestrator()
    try:
        results = orchestrator.run()
    finally:
        teardown()

    print(
        f"\nScenarios : {results.metrics.total_scenarios}  "
        f"Passed    : {results.metrics.passed_scenarios}  "
        f"Pass rate : {results.metrics.overall_pass_rate:.0%}"
    )
    results.as_table(show_workflow=True)
    results.agent_summary_table()

    print("\nDetail for 'plan_weeknight_dinner':")
    detail = results.get_scenario_detail("plan_weeknight_dinner")
    detail.traces_as_table()
    detail.tasks_as_table()
    detail.agent_results_as_table(show_tasks=True)

    print("\nDetail for 'debug_api_timeout':")
    detail = results.get_scenario_detail("debug_api_timeout")
    detail.traces_as_table()
    detail.tasks_as_table()
    detail.agent_results_as_table(show_tasks=True)


if __name__ == "__main__":
    main()
