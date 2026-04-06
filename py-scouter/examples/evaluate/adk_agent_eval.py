"""
Evaluating a Google ADK agent with EvalOrchestrator.

This example shows how to wrap an async ADK agent for offline evaluation:
  - Subclass EvalOrchestrator and override execute_agent to run async code.
  - Use ScouterInstrumentor to capture ADK spans automatically.
  - Use TraceAssertionTask to verify that expected spans were created.
  - Use AgentAssertionTask to verify tool calls inside those spans.

The simulated agent here mirrors what a real ADK agent would produce when
instrumented with ScouterInstrumentor. Replace the simulation with your actual
ADK Runner and the patterns carry over unchanged.

See also: py-scouter/examples/tracing/adk/main.py for production tracing setup.
"""

import asyncio

from google.adk.models.llm_response import LlmResponse
from google.genai import types
from scouter.agent import Provider
from scouter.drift import AgentEvalProfile
from scouter.evaluate import (
    AgentAssertion,
    AgentAssertionTask,
    AssertionTask,
    AttributeFilterTask,
    ComparisonOperator,
    EvalOrchestrator,
    EvalRecord,
    EvalScenario,
    EvalScenarios,
    MultiResponseMode,
    SpanFilter,
    TraceAssertion,
    TraceAssertionTask,
)
from scouter.queue import ScouterQueue
from scouter.tracing import ScouterInstrumentor, init_tracer
from scouter.transport import GrpcConfig

# ---------------------------------------------------------------------------
# Simulated ADK LlmResponse objects.
# A real ADK agent produces these internally; ScouterInstrumentor serialises
# them onto span attributes as gen_ai.response automatically.
# ---------------------------------------------------------------------------

# Routing decision: transfer to the RecipeAgent sub-agent
_ROUTER_RESPONSE = LlmResponse(
    model_version="gemini-2.0-flash",
    content=types.Content(
        role="model",
        parts=[
            types.Part(
                function_call=types.FunctionCall(
                    name="transfer_to_agent",
                    args={"agent_name": "RecipeAgent"},
                )
            )
        ],
    ),
    partial=False,
    finish_reason=types.FinishReason.STOP,
)

# Final text response from the RecipeAgent
_RECIPE_RESPONSE = LlmResponse(
    model_version="gemini-2.0-flash",
    content=types.Content(
        role="model",
        parts=[types.Part(text="Here is your recipe with step-by-step instructions.")],
    ),
    partial=False,
    finish_reason=types.FinishReason.STOP,
)

# ---------------------------------------------------------------------------
# 1. Profiles — one per sub-agent alias.
# ---------------------------------------------------------------------------

# The router profile verifies the routing span was created and the correct
# tool was called to hand off to the RecipeAgent.
router_profile = AgentEvalProfile(
    alias="router",
    tasks=[
        TraceAssertionTask(
            id="router_span_exists",
            assertion=TraceAssertion.span_count(SpanFilter.by_name("router.generate")),
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=1,
        ),
        TraceAssertionTask(
            id="transfer_called",
            assertion=TraceAssertion.attribute_filter(
                key="gen_ai.response",
                task=AttributeFilterTask.agent_assertion(
                    AgentAssertionTask(
                        id="transfer_to_recipe_agent",
                        assertion=AgentAssertion.tool_called("transfer_to_agent"),
                        expected_value=True,
                        operator=ComparisonOperator.Equals,
                        provider=Provider.GoogleAdk,
                    )
                ),
                mode=MultiResponseMode.Any,
            ),
            operator=ComparisonOperator.Equals,
            expected_value=True,
        ),
    ],
)

# The recipe agent profile verifies that a recipe was generated with steps.
recipe_profile = AgentEvalProfile(
    alias="recipe_agent",
    tasks=[
        AssertionTask(
            id="has_steps",
            context_path="has_steps",
            operator=ComparisonOperator.Equals,
            expected_value=True,
        ),
        AssertionTask(
            id="step_count",
            context_path="step_count",
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=3,
        ),
        TraceAssertionTask(
            id="recipe_span_exists",
            assertion=TraceAssertion.span_count(SpanFilter.by_name("recipe_agent.generate")),
            operator=ComparisonOperator.GreaterThanOrEqual,
            expected_value=1,
        ),
    ],
)

# ---------------------------------------------------------------------------
# 2. Queue and tracer.
#    ScouterInstrumentor is installed BEFORE the queue so that ADK's internal
#    OTEL calls are routed through Scouter automatically.
# ---------------------------------------------------------------------------
instrumentor = ScouterInstrumentor()

queue = ScouterQueue.from_profile(
    profile=[router_profile, recipe_profile],
    transport_config=GrpcConfig(),
)

instrumentor.instrument(scouter_queue=queue)

tracer = init_tracer(
    service_name="recipe-adk-agent",
    scouter_queue=queue,
    transport_config=GrpcConfig(),
)

# ---------------------------------------------------------------------------
# 3. Scenarios — one per recipe type.
# ---------------------------------------------------------------------------
_RECIPE_DATA = {
    "Give me a pasta recipe.": {
        "has_steps": True,
        "step_count": 5,
        "dish": "Aglio e Olio",
    },
    "Suggest a soup recipe.": {
        "has_steps": True,
        "step_count": 4,
        "dish": "Lentil Soup",
    },
    "What's a good dessert recipe?": {
        "has_steps": True,
        "step_count": 3,
        "dish": "Banana Ice Cream",
    },
}

scenarios = EvalScenarios(
    scenarios=[
        EvalScenario(
            id=f"recipe_{i}",
            initial_query=query,
            expected_outcome="A recipe with clear step-by-step instructions",
            tasks=[
                AssertionTask(
                    id="response_not_empty",
                    context_path="response",
                    operator=ComparisonOperator.IsString,
                    expected_value=True,
                ),
            ],
        )
        for i, query in enumerate(_RECIPE_DATA)
    ]
)


# ---------------------------------------------------------------------------
# 4. AdkEvalOrchestrator — bridges sync EvalOrchestrator with async ADK.
#
#    In production, self._runner would be your ADK Runner and self._session_service
#    would be an InMemorySessionService (or your session backend).
#    Here we simulate the spans and records an instrumented ADK run would produce.
# ---------------------------------------------------------------------------
class AdkEvalOrchestrator(EvalOrchestrator):
    """EvalOrchestrator subclass for async Google ADK agents.

    override execute_agent to run the async ADK event loop synchronously.
    In production, replace _simulate_adk_run with your actual ADK Runner call.
    """

    def execute_agent(self, scenario: EvalScenario) -> str:
        return asyncio.run(self._simulate_adk_run(scenario.initial_query))

    async def _simulate_adk_run(self, query: str) -> str:
        """Simulates the spans and records a real ADK run would produce.

        A real implementation would look like:
            session = await self._session_service.create_session(...)
            async for event in self._runner.run_async(...):
                if event.is_final_response():
                    return event.content.parts[0].text
        """
        data = _RECIPE_DATA[query]

        # Router span — ScouterInstrumentor captures this from ADK automatically.
        # We emit it explicitly here to simulate that behaviour.
        with tracer.start_as_current_span("router.generate") as span:
            span.set_attribute("gen_ai.response", _ROUTER_RESPONSE.model_dump_json())
            span.add_queue_item(
                "router",
                EvalRecord(
                    context={"routed_to": "RecipeAgent"},
                    id=f"router_{abs(hash(query)) % 10_000}",
                ),
            )

        # Recipe agent span — the sub-agent that does the actual work.
        with tracer.start_as_current_span("recipe_agent.generate") as span:
            span.set_attribute("gen_ai.response", _RECIPE_RESPONSE.model_dump_json())
            span.add_queue_item(
                "recipe_agent",
                EvalRecord(
                    context=data,
                    id=f"recipe_{abs(hash(query)) % 10_000}",
                ),
            )

        return f"[{data['dish']}] {_RECIPE_RESPONSE.content.parts[0].text}"  # type: ignore[union-attr,index]


def main() -> None:
    print("\n=== ADK Recipe Agent Evaluation ===\n")

    orchestrator = AdkEvalOrchestrator(
        queue=queue,
        scenarios=scenarios,
    )

    try:
        results = orchestrator.run()
    finally:
        instrumentor.uninstrument()

    print(
        f"\nScenarios: {results.metrics.total_scenarios}  "
        f"Passed: {results.metrics.passed_scenarios}  "
        f"Pass rate: {results.metrics.overall_pass_rate:.0%}"
    )
    results.as_table(show_workflow=True)


if __name__ == "__main__":
    main()
