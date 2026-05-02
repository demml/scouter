"""Mock interactive agent — mirrors google/agent.py structure, zero LLM cost."""

from __future__ import annotations

from scouter import trace
from scouter.agent.mock import AfterModelCallback, MockTool, ScouterMockAgent
from scouter.evaluate import EvalRecord

from ..shared import get_shared_config

config = get_shared_config()

_ALIAS = "interactive_support_agent"

_RESPONSES: dict[str, str] = {
    "dinner": (
        "Step 1: Choose one protein (chicken, eggs, or beans). "
        "Step 2: Add one vegetable. "
        "Step 3: Add one starch (rice or pasta). "
        "Risk: forgetting to defrost the protein. DONE"
    ),
    "timeout": (
        "Step 1: Check your client timeout config. "
        "Step 2: Add retry logic with exponential backoff. "
        "Step 3: Profile downstream dependency latency. "
        "Risk: cascading timeouts if retries are unbounded. DONE"
    ),
}
_DEFAULT_RESPONSE = "I can help with that. Step 1: gather requirements. DONE"


def _route_response(query: str) -> str:
    lowered = query.lower()
    for keyword, response in _RESPONSES.items():
        if keyword in lowered:
            return response
    return _DEFAULT_RESPONSE


def _emit_eval_record(query: str, response: str) -> None:
    tracer = trace.get_tracer("evaluate.agent.mock")
    with tracer.start_as_current_span("mock.callback") as span:
        span.add_queue_item(
            _ALIAS,
            EvalRecord(context={"query": query, "response": response}),
        )


def _mock_lookup(query: str) -> str:
    return f"lookup result for: {query[:60]}"


class MockAgentService:
    def __init__(self, callback: AfterModelCallback | None = None) -> None:
        self._agent = ScouterMockAgent(
            alias=_ALIAS,
            model_name="mock-model-1.0",
            provider="mock",
            tools=[MockTool(name="mock_lookup", fn=_mock_lookup)],
            response_fn=_route_response,
            after_model_callback=callback or _emit_eval_record,
        )

    def run(self, query: str) -> str:
        return self._agent.run(query)


def build_agent_service(callback: AfterModelCallback | None = None) -> MockAgentService:
    return MockAgentService(callback=callback)
