"""
Reactive evaluation entry point — run this file to execute the eval.

Two ADK agents work together:
- recipe_agent: the service under evaluation. Maintains a persistent session
  per scenario so it remembers the full conversation context across turns.
- customer_agent: simulates a home cook user. Called statelessly each turn —
  it only needs the original goal and the last response to decide whether to
  ask a follow-up or signal completion.

The eval loop runs until customer_agent outputs "SATISFIED" or max_turns is reached.
All EvalRecords emitted by recipe_agent's callback across every turn are captured
and evaluated together. Only the final response is used for scenario-level tasks.

Run: uv run python -m examples.evaluate.react.adk.eval
"""

import asyncio
from typing import Optional

from google.adk.runners import Runner
from google.adk.sessions import InMemorySessionService
from google.genai import types
from scouter.evaluate import EvalOrchestrator, EvalScenario, EvalScenarios
from scouter.queue import ScouterQueue

from .agents.customer import customer_agent
from .agents.recipe import recipe_agent
from .scenarios import scenarios
from .setup import config


class RecipeReactOrchestrator(EvalOrchestrator):
    """Drives a reactive recipe conversation between two ADK agents.

    recipe_agent maintains session state across turns so it accumulates
    conversation context. customer_agent is called statelessly — each turn
    it receives the original goal and the last response and decides what to
    ask next, or outputs the termination signal when satisfied.
    """

    def __init__(
        self,
        recipe_runner: Runner,
        customer_runner: Runner,
        session_service: InMemorySessionService,
        queue: ScouterQueue,
        eval_scenarios: EvalScenarios,
    ) -> None:
        super().__init__(queue=queue, scenarios=eval_scenarios)
        self._recipe_runner = recipe_runner
        self._customer_runner = customer_runner
        self._session_service = session_service
        self._recipe_session_id: Optional[str] = None

    def on_scenario_start(self, scenario: EvalScenario) -> None:
        session = asyncio.run(
            self._session_service.create_session(
                app_name="recipe_app",
                user_id="eval_user",
            )
        )
        self._recipe_session_id = session.id

    def execute_agent_turn(self, scenario: EvalScenario, message: str) -> str:
        return asyncio.run(self._run_recipe_turn(message))

    def execute_simulated_user_turn(
        self,
        scenario: EvalScenario,
        initial_query: str,
        agent_response: str,
    ) -> str:
        return asyncio.run(self._run_customer_turn(initial_query, agent_response))

    async def _run_recipe_turn(self, message: str) -> str:
        content = types.Content(role="user", parts=[types.Part(text=message)])
        response_text = ""
        async for event in self._recipe_runner.run_async(
            user_id="eval_user",
            session_id=self._recipe_session_id,
            new_message=content,
        ):
            if event.is_final_response() and event.content:
                for part in event.content.parts:  # type: ignore
                    if part.text:
                        response_text = part.text
                        break
        return response_text

    async def _run_customer_turn(self, initial_query: str, agent_response: str) -> str:
        prompt = (
            f"Original goal: {initial_query}\n"
            f"Cooking assistant's response: {agent_response}\n\n"
            "What do you say next?"
        )
        session = await self._session_service.create_session(
            app_name="customer_app",
            user_id="customer_user",
        )
        content = types.Content(role="user", parts=[types.Part(text=prompt)])
        response_text = ""
        async for event in self._customer_runner.run_async(
            user_id="customer_user",
            session_id=session.id,
            new_message=content,
        ):
            if event.is_final_response() and event.content:
                for part in event.content.parts:  # type: ignore
                    if part.text:
                        response_text = part.text
                        break
        return response_text


def main() -> None:
    session_service = InMemorySessionService()
    recipe_runner = Runner(
        agent=recipe_agent,
        app_name="recipe_app",
        session_service=session_service,
    )
    customer_runner = Runner(
        agent=customer_agent,
        app_name="customer_app",
        session_service=session_service,
    )

    try:
        results = RecipeReactOrchestrator(
            recipe_runner=recipe_runner,
            customer_runner=customer_runner,
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
