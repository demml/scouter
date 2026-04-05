"""
Multi-turn evaluation with EvalOrchestrator.

This example evaluates a cooking assistant agent across multiple scenarios,
each involving a short back-and-forth conversation (initial query + follow-up
turns). It also shows how to use lifecycle hooks by subclassing EvalOrchestrator.

Key concepts demonstrated:
  - predefined_turns: scripted follow-up queries after the initial question
  - Multiple EvalScenarios with per-scenario assertion tasks
  - on_scenario_start / on_scenario_complete / on_evaluation_complete hooks
  - Inspecting per-scenario and aggregate metrics

Replace GrpcConfig() and ChefAgent with your real setup.
"""

from scouter.drift import AgentEvalProfile
from scouter.evaluate import (
    AssertionTask,
    ComparisonOperator,
    EvalOrchestrator,
    EvalRecord,
    EvalScenario,
    EvalScenarios,
    ScenarioEvalResults,
)
from scouter.queue import ScouterQueue
from scouter.tracing import init_tracer
from scouter.transport import GrpcConfig

# ---------------------------------------------------------------------------
# 1. Profile — assertions run against every EvalRecord your agent emits.
# ---------------------------------------------------------------------------
profile = AgentEvalProfile(
    alias="chef_agent",
    tasks=[
        AssertionTask(
            id="recipe_name_present",
            context_path="recipe_name",
            operator=ComparisonOperator.IsString,
            expected_value=True,
        ),
        AssertionTask(
            id="step_count_sufficient",
            context_path="step_count",
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=3,
        ),
    ],
)

queue = ScouterQueue.from_profile(
    profile=[profile],
    transport_config=GrpcConfig(),
)

tracer = init_tracer(
    service_name="chef-agent",
    scouter_queue=queue,
    transport_config=GrpcConfig(),
)

# ---------------------------------------------------------------------------
# 2. Multi-turn scenarios.
#    agent_fn is called once for initial_query, then once per predefined_turn.
#    EvalOrchestrator captures records from all turns under the same scenario.
# ---------------------------------------------------------------------------
scenarios = EvalScenarios(
    scenarios=[
        EvalScenario(
            id="pasta_scenario",
            initial_query="Give me a quick pasta recipe.",
            predefined_turns=[
                "Make it vegetarian.",
                "Cut it down to under 30 minutes.",
            ],
            expected_outcome="A short vegetarian pasta recipe",
            tasks=[
                AssertionTask(
                    id="mentions_pasta",
                    context_path="response",
                    operator=ComparisonOperator.Contains,
                    expected_value="pasta",
                ),
            ],
        ),
        EvalScenario(
            id="soup_scenario",
            initial_query="Suggest a hearty soup for a cold day.",
            predefined_turns=[
                "Make it vegan.",
                "What can I prep ahead of time?",
            ],
            expected_outcome="A vegan soup with prep-ahead tips",
            tasks=[
                AssertionTask(
                    id="mentions_soup",
                    context_path="response",
                    operator=ComparisonOperator.Contains,
                    expected_value="soup",
                ),
            ],
        ),
        EvalScenario(
            id="dessert_scenario",
            initial_query="What's an easy dessert I can make tonight?",
            predefined_turns=[
                "I only have 20 minutes.",
                "No oven — stovetop only.",
            ],
            expected_outcome="A quick stovetop dessert",
            tasks=[
                AssertionTask(
                    id="response_not_empty",
                    context_path="response",
                    operator=ComparisonOperator.IsString,
                    expected_value=True,
                ),
            ],
        ),
        EvalScenario(
            id="breakfast_scenario",
            initial_query="Recommend a protein-rich breakfast.",
            predefined_turns=[
                "Make it dairy-free.",
                "Can I meal-prep it for the week?",
            ],
            expected_outcome="A dairy-free, meal-preppable high-protein breakfast",
            tasks=[
                AssertionTask(
                    id="mentions_protein",
                    context_path="response",
                    operator=ComparisonOperator.Contains,
                    expected_value="protein",
                ),
            ],
        ),
    ]
)

# ---------------------------------------------------------------------------
# 3. Simulated chef agent.
#    Each turn gets a canned response. A real agent would maintain conversation
#    history and refine its response across turns.
# ---------------------------------------------------------------------------
_RESPONSES: dict[str, str] = {
    # pasta scenario
    "Give me a quick pasta recipe.": (
        "Here's a classic pasta: boil spaghetti, toss with olive oil, garlic, and parmesan. Done in 20 minutes."
    ),
    "Make it vegetarian.": "It's already vegetarian! Swap parmesan for nutritional yeast if you want it vegan too.",
    "Cut it down to under 30 minutes.": "This pasta takes about 15 minutes total — well under 30.",
    # soup scenario
    "Suggest a hearty soup for a cold day.": (
        "Try a lentil soup: onion, carrot, celery, lentils, vegetable broth. Simmer 30 minutes."
    ),
    "Make it vegan.": "Good news — it's already vegan. No dairy or meat involved.",
    "What can I prep ahead of time?": "Chop the vegetables and portion the lentils the night before. The soup reheats perfectly.",
    # dessert scenario
    "What's an easy dessert I can make tonight?": "Chocolate mousse, banana ice cream, or stovetop rice pudding all work well.",
    "I only have 20 minutes.": "Banana ice cream takes 5 minutes: blend frozen bananas until smooth. That's it.",
    "No oven — stovetop only.": "Banana ice cream is stovetop-free. Rice pudding simmers on the stovetop in 20 minutes.",
    # breakfast scenario
    "Recommend a protein-rich breakfast.": "Scrambled eggs with smoked salmon, or a Greek yogurt parfait with nuts and seeds.",
    "Make it dairy-free.": "Skip the yogurt — try a tofu scramble with black beans and salsa. High protein, dairy-free.",
    "Can I meal-prep it for the week?": "Yes! Cook a batch of tofu scramble on Sunday. Reheat each morning in under 2 minutes.",
}

_SCENARIO_CONTEXTS: dict[str, dict] = {
    "pasta": {"recipe_name": "Aglio e Olio", "step_count": 4},
    "soup": {"recipe_name": "Red Lentil Soup", "step_count": 5},
    "dessert": {"recipe_name": "Banana Ice Cream", "step_count": 3},
    "breakfast": {"recipe_name": "Tofu Scramble", "step_count": 4},
}


def agent_fn(query: str) -> str:
    response = _RESPONSES.get(query, "I can help with that recipe!")

    # Determine which scenario we're in from the query keywords
    domain = next((k for k in _SCENARIO_CONTEXTS if k in query.lower()), "pasta")
    ctx = _SCENARIO_CONTEXTS[domain]

    with tracer.start_as_current_span("chef_agent_call") as span:
        span.add_queue_item(
            "chef_agent",
            EvalRecord(
                context={**ctx, "last_query": query},
                id=f"chef_{abs(hash(query)) % 10_000}",
            ),
        )

    return response


# ---------------------------------------------------------------------------
# 4. Subclass EvalOrchestrator to add lifecycle hooks.
#    Override any of the three hooks to inspect or log state at each stage.
# ---------------------------------------------------------------------------
class ChefEvalOrchestrator(EvalOrchestrator):
    def on_scenario_start(self, scenario: EvalScenario) -> None:
        turns = len(scenario.predefined_turns)
        print(f"  → Running '{scenario.id}' ({1 + turns} turns)")

    def on_scenario_complete(self, scenario: EvalScenario, response: str) -> None:
        print(f"  ✓ '{scenario.id}' complete — final response: {response[:60]!r}")

    def on_evaluation_complete(self, results: ScenarioEvalResults) -> ScenarioEvalResults:
        passed = results.metrics.passed_scenarios
        total = results.metrics.total_scenarios
        print(f"\n  Evaluation complete: {passed}/{total} scenarios passed")
        return results


def main() -> None:
    print("\n=== Multi-Turn Chef Agent Evaluation ===\n")

    orchestrator = ChefEvalOrchestrator(
        queue=queue,
        scenarios=scenarios,
        agent_fn=agent_fn,
    )
    results = orchestrator.run()

    print(f"\nOverall pass rate : {results.metrics.overall_pass_rate:.0%}")
    print(f"Scenario pass rate: {results.metrics.scenario_pass_rate:.0%}")
    print()
    results.as_table(show_workflow=True)


if __name__ == "__main__":
    main()
