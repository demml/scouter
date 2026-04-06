"""
Evaluation entry point — run this file to execute the eval.

Wires together setup, agent, and scenarios into an EvalOrchestrator subclass
that drives the ADK Runner once per scenario and returns ScenarioEvalResults.
"""

import asyncio

from google.adk.runners import Runner
from google.adk.sessions import InMemorySessionService
from google.genai import types
from scouter.evaluate import EvalOrchestrator, EvalScenario, EvalScenarios
from scouter.queue import ScouterQueue

from .agent import qa_agent
from .scenarios import scenarios
from .setup import config


class QaEvalOrchestrator(EvalOrchestrator):
    """Bridges the synchronous EvalOrchestrator loop with the async ADK Runner.

    execute_agent is called once per scenario. Override it to drive the
    ADK Runner and return the agent's final response string.
    """

    def __init__(
        self,
        runner: Runner,
        session_service: InMemorySessionService,
        queue: ScouterQueue,
        eval_scenarios: EvalScenarios,
    ) -> None:
        super().__init__(queue=queue, scenarios=eval_scenarios)
        self._runner = runner
        self._session_service = session_service

    def execute_agent(self, scenario: EvalScenario) -> str:
        return asyncio.run(self._run_query(scenario.initial_query))

    async def _run_query(self, query: str) -> str:
        session = await self._session_service.create_session(app_name="qa_app", user_id="eval_user")
        message = types.Content(role="user", parts=[types.Part(text=query)])
        response_text = ""
        async for event in self._runner.run_async(
            user_id="eval_user",
            session_id=session.id,
            new_message=message,
        ):
            if event.is_final_response() and event.content:
                for part in event.content.parts:  # type: ignore
                    if part.text:
                        response_text = part.text
                        break
        return response_text


def main() -> None:
    session_service = InMemorySessionService()
    runner = Runner(
        agent=qa_agent,
        app_name="qa_app",
        session_service=session_service,
    )

    try:
        results = QaEvalOrchestrator(
            runner=runner,
            session_service=session_service,
            queue=config.queue,
            eval_scenarios=scenarios,
        ).run()
    finally:
        config.instrumentor.uninstrument()

    print(
        f"\nScenarios : {results.metrics.total_scenarios}  "
        f"Passed    : {results.metrics.passed_scenarios}  "
        f"Pass rate : {results.metrics.overall_pass_rate:.0%}"
    )
    results.as_table(show_workflow=True)


if __name__ == "__main__":
    main()
