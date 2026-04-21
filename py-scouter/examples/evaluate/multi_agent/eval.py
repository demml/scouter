"""
Multi-agent evaluation entry point.

Runs each scenario in scenarios.jsonl through a two-agent CrewAI crew
(researcher → analyst). Each agent emits an EvalRecord via its task
callback, feeding Level 1 sub-agent evaluation. The crew's final output
(analyst response) is used for Level 2 scenario-level evaluation.

Usage:
    cd py-scouter
    uv run python -m examples.evaluate.multi_agent.eval
"""

from crewai.crews.crew_output import CrewOutput
from scouter.evaluate import EvalOrchestrator, EvalScenario, EvalScenarios
from scouter.queue import ScouterQueue

from .agents import build_crew
from .scenarios import scenarios
from .setup import config


class MultiAgentEvalOrchestrator(EvalOrchestrator):
    def __init__(
        self,
        queue: ScouterQueue,
        eval_scenarios: EvalScenarios,
    ) -> None:
        super().__init__(queue=queue, scenarios=eval_scenarios)

    def execute_agent(self, scenario: EvalScenario) -> str:
        result = build_crew(
            scenario.initial_query,
            config.researcher_prompt,
            config.analyst_prompt,
        ).kickoff()
        assert isinstance(result, CrewOutput)
        return result.raw


def main() -> None:
    try:
        results = MultiAgentEvalOrchestrator(
            queue=config.queue,
            eval_scenarios=scenarios,
        ).run()
    finally:
        config.crewai_instrumentor.uninstrument()
        config.instrumentor.uninstrument()

    print(
        f"\nScenarios : {results.metrics.total_scenarios}  "
        f"Passed    : {results.metrics.passed_scenarios}  "
        f"Pass rate : {results.metrics.overall_pass_rate:.0%}"
    )
    results.as_table(show_workflow=True)


if __name__ == "__main__":
    main()
