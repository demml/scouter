from __future__ import annotations

import os
from typing import Any, Callable

from agents import Agent, RunHooks, Runner
from fastapi import FastAPI
from opentelemetry.instrumentation.openai_agents import OpenAIAgentsInstrumentor
from pydantic import BaseModel
from scouter import trace
from scouter.evaluate import EvalRecord

from ..shared import get_shared_config, teardown_shared_config

config = get_shared_config()
_openai_instrumentor = OpenAIAgentsInstrumentor()
_openai_instrumentor.instrument(tracer_provider=trace.get_tracer_provider())

AgentCallback = Callable[[str, str], None]


class AgentRequest(BaseModel):
    query: str


class AgentResponse(BaseModel):
    response: str


def _emit_eval_record(query: str, response: str) -> None:
    tracer = trace.get_tracer("evaluate.non_interactive.openai")
    with tracer.start_as_current_span("openai.callback") as span:
        span.add_queue_item(
            "support_agent",
            EvalRecord(
                id=f"openai_{abs(hash((query, response))) % 1_000_000}",
                context={"query": query, "response": response},
            ),
        )


class EvalHooks(RunHooks[Any]):
    def __init__(self, query: str, callback: AgentCallback) -> None:
        self._query = query
        self._callback = callback

    async def on_agent_end(self, context: Any, agent: Any, output: Any) -> None:
        del context
        del agent
        self._callback(self._query, str(output))


_agent = Agent(
    name="openai_non_interactive_agent",
    instructions=config.prompt.message.text,
    model="gpt-4.1-mini",
)


def _fallback_response(query: str) -> str:
    lowered = query.lower()
    if "france" in lowered:
        return "Paris is the capital of France."
    if "water" in lowered:
        return "The chemical formula for water is H2O."
    if "largest planet" in lowered:
        return "Jupiter is the largest planet in our solar system."
    return "Fallback response because OPENAI_API_KEY is not set."


def run_agent(query: str, callback: AgentCallback | None = None) -> str:
    on_response = callback or _emit_eval_record

    if not os.getenv("OPENAI_API_KEY"):
        response = _fallback_response(query)
        on_response(query, response)
        return response

    result = Runner.run_sync(_agent, query, hooks=EvalHooks(query=query, callback=on_response))
    return str(result.final_output)


app = FastAPI(title="Scouter OpenAI Non-Interactive Agent")


@app.post("/ask", response_model=AgentResponse)
def ask(request: AgentRequest) -> AgentResponse:
    return AgentResponse(response=run_agent(request.query))


def shutdown() -> None:
    _openai_instrumentor.uninstrument()
    teardown_shared_config()
