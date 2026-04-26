"""Offline evaluation entrypoint for the Google ADK example.

The important behavior here is lifecycle management:

- one `GoogleAgentService`
- one persistent event loop
- one `EvalOrchestrator`

That keeps ADK tracing cleanup on a stable loop instead of creating a fresh
`asyncio.run()` context for every scenario.
"""

from __future__ import annotations

import asyncio

from scouter.evaluate import EvalOrchestrator, EvalScenario

from ..shared import get_shared_config, teardown_shared_config
from .agent import GoogleAgentService, build_agent_service


class GoogleEvalOrchestrator(EvalOrchestrator):
    """Bridge the sync eval runner to one persistent ADK async runtime."""

    def __init__(self) -> None:
        config = get_shared_config()
        super().__init__(queue=config.queue, scenarios=config.scenarios)
        self._runner = asyncio.Runner()
        self._service: GoogleAgentService = build_agent_service()

    def execute_agent(self, scenario: EvalScenario) -> str:
        """Run each scenario on the same event loop and service instance."""
        return self._runner.run(self._service.run(scenario.initial_query))

    def close(self) -> None:
        """Close the loop after evaluation completes."""
        self._runner.close()


def main() -> None:
    """Run the shared non-interactive scenarios against the Google example."""
    orchestrator = GoogleEvalOrchestrator()
    try:
        results = orchestrator.run()
    finally:
        orchestrator.close()
        teardown_shared_config()

    results.as_table(show_workflow=True)

    results.get_scenario_detail(
        "largest_planet",
    ).traces_as_table()


if __name__ == "__main__":
    main()
