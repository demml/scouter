"""Interactive evaluation entrypoint for the Google ADK example.

This keeps the ADK runtime on one event loop for the full evaluation and
reuses the same service object that powers the FastAPI example.
"""

from __future__ import annotations

import asyncio
from typing import Any

from scouter.evaluate import EvalOrchestrator, EvalScenario

from ..shared import get_shared_config, teardown_shared_config
from .agent import GoogleAgentService, build_agent_service


def simulated_user_turn(
    initial_query: str,
    agent_response: Any,
    history: list[dict[str, Any]],
) -> str:
    """Drive a short reactive conversation for the shared interactive scenarios."""
    del initial_query

    if len(history) >= 2:
        return "DONE"

    response_text = str(agent_response).lower()
    if "step" not in response_text:
        return "Give me concrete step-by-step actions and end with DONE when complete."
    return "Add one risk to watch for and then return DONE."


class GoogleInteractiveEvalOrchestrator(EvalOrchestrator):
    """Bridge the sync eval runner to one persistent ADK async runtime."""

    def __init__(self) -> None:
        config = get_shared_config()
        super().__init__(
            queue=config.queue,
            scenarios=config.scenarios,
            simulated_user_fn=simulated_user_turn,
        )
        self._runner = asyncio.Runner()
        self._service: GoogleAgentService = build_agent_service()

    def execute_agent_turn(self, scenario: EvalScenario, message: str) -> str:
        """Run each reactive turn on the same event loop and service instance."""
        del scenario
        return self._runner.run(self._service.run(message))

    def close(self) -> None:
        """Close the loop after evaluation completes."""
        self._runner.close()


def main() -> None:
    """Run the shared interactive scenarios against the Google example."""
    orchestrator = GoogleInteractiveEvalOrchestrator()
    try:
        results = orchestrator.run()
    finally:
        orchestrator.close()
        teardown_shared_config()

    print(
        f"\nScenarios : {results.metrics.total_scenarios}  "
        f"Passed    : {results.metrics.passed_scenarios}  "
        f"Pass rate : {results.metrics.overall_pass_rate:.0%}"
    )
    results.as_table(show_workflow=True)


if __name__ == "__main__":
    main()
